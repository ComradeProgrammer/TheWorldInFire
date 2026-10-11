//! WebAssembly plugin runtime.
//!
//! Plugins are compiled once per process with Cranelift and instantiated once
//! per game. All instances of a game share one [`Store`], whose data holds the
//! kernel's authoritative state, so a host import running for one plugin can
//! apply its changes and call the other plugins taking part in a filter.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Once, OnceLock};
use std::time::Duration;

use ooaw_plugin_api::abi::{self, exports, imports, ABI_VERSION};
use ooaw_plugin_api::protocol::{
    Attachment, Change, PluginManifest, ScenarioRequest, ScenarioSetup,
};
use ooaw_plugin_api::serde_json::Value;
use ooaw_plugin_api::{GameId, GameStatus, HexId, PhaseDefinition, RuleError};
use serde::de::DeserializeOwned;
use wasmtime::{
    AsContextMut, Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreContextMut,
    StoreLimits, StoreLimitsBuilder, TypedFunc,
};

use crate::dice::Dice;
use crate::state::KernelState;

/// Interval at which the guarded engine's epoch advances.
const EPOCH_TICK: Duration = Duration::from_millis(100);

/// Epoch ticks one top-level call into a guarded game, including the filters
/// it runs, may take before it is stopped. Ordinary calls take milliseconds;
/// the deadline only stops a plugin that never returns. Being wall-clock
/// based, it never changes the result of a call that finishes.
const CALL_DEADLINE_TICKS: u64 = 100;

/// Maximum linear memory of one plugin instance.
const MEMORY_LIMIT: usize = 512 << 20;

/// The official NATO rules plugin, built from `crates/plugins/nato-official` by this
/// crate's build script.
static OFFICIAL_PLUGIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ooaw_nato.wasm"));

/// How the plugins of a game are sandboxed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sandbox {
    /// Only plugins bundled with the kernel: compiled without deadline checks,
    /// which would cost up to half again the running time.
    Trusted,
    /// At least one third-party plugin: every call has a wall-clock deadline.
    Guarded,
}

/// Engine configuration shared by both sandboxes.
fn base_config() -> Config {
    let mut config = Config::new();
    // Determinism: NaN bit patterns and relaxed SIMD may otherwise differ
    // between machines. Threads are not compiled in at all.
    config
        .cranelift_nan_canonicalization(true)
        .wasm_relaxed_simd(false);
    config
}

/// The process-wide Wasmtime engine for a sandbox.
fn engine(sandbox: Sandbox) -> &'static Engine {
    static TRUSTED: OnceLock<Engine> = OnceLock::new();
    static GUARDED: OnceLock<Engine> = OnceLock::new();
    static TICKER: Once = Once::new();
    match sandbox {
        Sandbox::Trusted => TRUSTED.get_or_init(|| {
            Engine::new(&base_config()).expect("the plugin engine configuration is valid")
        }),
        Sandbox::Guarded => {
            let engine = GUARDED.get_or_init(|| {
                let mut config = base_config();
                config.epoch_interruption(true);
                Engine::new(&config).expect("the plugin engine configuration is valid")
            });
            TICKER.call_once(|| {
                std::thread::Builder::new()
                    .name("ooaw-plugin-epoch".to_owned())
                    .spawn(move || loop {
                        std::thread::sleep(EPOCH_TICK);
                        engine.increment_epoch();
                    })
                    .expect("can start the plugin deadline thread");
            });
            engine
        }
    }
}

/// A plugin and its manifest, shared by every game that loads it.
pub struct PluginModule {
    /// WebAssembly bytes, kept to compile for the guarded sandbox on demand.
    bytes: Vec<u8>,
    /// Whether the plugin is bundled with the kernel.
    trusted: bool,
    /// Code compiled for the trusted sandbox; only for trusted plugins.
    trusted_module: OnceLock<Module>,
    /// Code compiled for the guarded sandbox.
    guarded_module: OnceLock<Module>,
    /// What the plugin provides.
    manifest: PluginManifest,
}

impl std::fmt::Debug for PluginModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginModule")
            .field("manifest", &self.manifest)
            .field("trusted", &self.trusted)
            .finish_non_exhaustive()
    }
}

fn load_error(message: String) -> RuleError {
    RuleError::new("pluginLoad", message)
}

fn compile(sandbox: Sandbox, bytes: &[u8]) -> Result<Module, RuleError> {
    Module::from_binary(engine(sandbox), bytes)
        .map_err(|error| load_error(format!("Invalid plugin module: {error}")))
}

impl PluginModule {
    /// Compiles a third-party plugin from WebAssembly bytes and reads its
    /// manifest. Games that load it run every plugin call under a deadline.
    ///
    /// # Errors
    ///
    /// Returns `pluginLoad` when the bytes are not a valid plugin module or the
    /// plugin was built for a different ABI version.
    pub fn from_wasm(bytes: &[u8]) -> Result<Arc<Self>, RuleError> {
        Self::load(bytes, false)
    }

    fn load(bytes: &[u8], trusted: bool) -> Result<Arc<Self>, RuleError> {
        let sandbox = if trusted {
            Sandbox::Trusted
        } else {
            Sandbox::Guarded
        };
        let module = compile(sandbox, bytes)?;
        let manifest = read_manifest(sandbox, &module)?;
        let plugin = Self {
            bytes: bytes.to_vec(),
            trusted,
            trusted_module: OnceLock::new(),
            guarded_module: OnceLock::new(),
            manifest,
        };
        let slot = match sandbox {
            Sandbox::Trusted => &plugin.trusted_module,
            Sandbox::Guarded => &plugin.guarded_module,
        };
        let _ = slot.set(module);
        Ok(Arc::new(plugin))
    }

    /// What the plugin provides.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Whether the plugin is bundled with the kernel and runs without deadlines.
    pub fn is_trusted(&self) -> bool {
        self.trusted
    }

    /// The plugin's code compiled for a sandbox, compiling it on first use.
    fn module(&self, sandbox: Sandbox) -> Result<&Module, RuleError> {
        let slot = match sandbox {
            Sandbox::Trusted if self.trusted => &self.trusted_module,
            Sandbox::Trusted => {
                return Err(load_error(format!(
                    "Plugin {} is not trusted",
                    self.manifest.id
                )))
            }
            Sandbox::Guarded => &self.guarded_module,
        };
        if slot.get().is_none() {
            let _ = slot.set(compile(sandbox, &self.bytes)?);
        }
        Ok(slot.get().expect("set above"))
    }
}

/// Instantiates a module outside any game and asks for its manifest.
fn read_manifest(sandbox: Sandbox, module: &Module) -> Result<PluginManifest, RuleError> {
    let mut store = new_store(HostData::detached(sandbox));
    let instance = linker(sandbox)
        .instantiate(&mut store, module)
        .map_err(|error| load_error(format!("Cannot instantiate plugin: {error}")))?;
    let version = instance
        .get_typed_func::<(), u32>(&mut store, exports::VERSION)
        .and_then(|func| func.call(&mut store, ()))
        .map_err(|error| load_error(format!("Plugin exports no ABI version: {error}")))?;
    if version != ABI_VERSION {
        return Err(load_error(format!(
            "Plugin uses ABI version {version}, but the kernel supports {ABI_VERSION}"
        )));
    }
    let exports = Exports::of(&instance, &mut store).map_err(load_error)?;
    let raw = exports
        .manifest
        .call(&mut store, ())
        .and_then(|packed| read_result(&mut store.as_context_mut(), &exports, packed))
        .map_err(|error| load_error(format!("Plugin failed to describe itself: {error}")))?;
    let manifest: PluginManifest = abi::decode(&raw)
        .map_err(|error| load_error(format!("Malformed manifest: {}", error.message)))?;
    if manifest.abi_version != ABI_VERSION {
        return Err(load_error(format!(
            "Plugin {} declares ABI version {}, but the kernel supports {ABI_VERSION}",
            manifest.id, manifest.abi_version
        )));
    }
    Ok(manifest)
}

/// Returns the bundled official NATO rules plugin, compiling it on first use.
///
/// # Errors
///
/// Returns `pluginLoad` if the bundled module cannot be compiled, which
/// indicates a broken build.
pub fn official_plugin() -> Result<Arc<PluginModule>, RuleError> {
    static OFFICIAL: OnceLock<Result<Arc<PluginModule>, RuleError>> = OnceLock::new();
    OFFICIAL
        .get_or_init(|| PluginModule::load(OFFICIAL_PLUGIN_WASM, true))
        .clone()
}

/// An export that takes one encoded argument and returns an encoded result.
type MethodFunc = TypedFunc<(u32, u32), u64>;

/// Exports of one plugin instance; see [`exports`] for their signatures.
#[derive(Clone)]
struct Exports {
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    free: TypedFunc<(u32, u32), ()>,
    manifest: TypedFunc<(), u64>,
    set_turn: TypedFunc<(u32, u32, u32), ()>,
    sync: MethodFunc,
    take_changes: TypedFunc<(), u64>,
    create_game: MethodFunc,
    attach: MethodFunc,
    apply_setup: MethodFunc,
    phase_started: MethodFunc,
    phase_ending: MethodFunc,
    command: MethodFunc,
    query: MethodFunc,
    filter: MethodFunc,
}

impl Exports {
    fn of(instance: &wasmtime::Instance, mut store: impl AsContextMut) -> Result<Self, String> {
        let mut store = store.as_context_mut();
        let memory = instance
            .get_memory(&mut store, exports::MEMORY)
            .ok_or("Plugin exports no memory")?;
        macro_rules! func {
            ($name:expr) => {
                instance
                    .get_typed_func(&mut store, $name)
                    .map_err(|error| format!("Plugin export {}: {error}", $name))?
            };
        }
        Ok(Self {
            memory,
            alloc: func!(exports::ALLOC),
            free: func!(exports::FREE),
            manifest: func!(exports::MANIFEST),
            set_turn: func!(exports::SET_TURN),
            sync: func!(exports::SYNC),
            take_changes: func!(exports::TAKE_CHANGES),
            create_game: func!(exports::CREATE_GAME),
            attach: func!(exports::ATTACH),
            apply_setup: func!(exports::APPLY_SETUP),
            phase_started: func!(exports::PHASE_STARTED),
            phase_ending: func!(exports::PHASE_ENDING),
            command: func!(exports::COMMAND),
            query: func!(exports::QUERY),
            filter: func!(exports::FILTER),
        })
    }

    fn method(&self, method: Method) -> &MethodFunc {
        match method {
            Method::CreateGame => &self.create_game,
            Method::Attach => &self.attach,
            Method::ApplySetup => &self.apply_setup,
            Method::PhaseStarted => &self.phase_started,
            Method::PhaseEnding => &self.phase_ending,
            Method::Command => &self.command,
            Method::Query => &self.query,
            Method::Filter => &self.filter,
        }
    }
}

/// A [`RulesPlugin`](ooaw_plugin_api::sdk::RulesPlugin) method the kernel calls
/// with an encoded argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    CreateGame,
    Attach,
    ApplySetup,
    PhaseStarted,
    PhaseEnding,
    Command,
    Query,
    Filter,
}

impl Method {
    /// Whether the call starts the plugin's mirror afresh, so it needs no
    /// sync before and reports no changes after.
    fn resets_mirror(self) -> bool {
        matches!(self, Self::CreateGame | Self::Attach)
    }
}

/// One plugin loaded into a game.
pub(crate) struct Slot {
    /// Compiled plugin.
    pub(crate) plugin: Arc<PluginModule>,
    /// Live instance, or `None` before instantiation and after a fault.
    exports: Option<Exports>,
    /// Kernel state generation the instance's mirror matches, if any.
    synced: Option<u64>,
    /// The game a fresh instance is attached to.
    pub(crate) attach: Option<Attachment>,
}

impl Slot {
    pub(crate) fn new(plugin: Arc<PluginModule>) -> Self {
        Self {
            plugin,
            exports: None,
            synced: None,
            attach: None,
        }
    }
}

/// Store data for one game: authoritative state plus plugin bookkeeping.
pub(crate) struct HostData {
    /// How the game's plugins are sandboxed.
    pub(crate) sandbox: Sandbox,
    /// Authoritative mutable state.
    pub(crate) state: KernelState,
    /// The game's only dice.
    pub(crate) dice: Dice,
    /// Incremented whenever the state changes other than through a plugin's
    /// own reported changes, invalidating every plugin's mirror.
    pub(crate) generation: u64,
    /// Hexes of the map, used to check plugin changes.
    pub(crate) hexes: HashSet<HexId>,
    /// Loaded plugins in load order.
    pub(crate) slots: Vec<Slot>,
    /// Filter name to taking-part plugins, in load order.
    pub(crate) filters: HashMap<String, Vec<usize>>,
    /// Whether the current call may change state or roll dice.
    pub(crate) read_only: bool,
    /// Plugins currently handling a call, outermost first.
    stack: Vec<usize>,
    /// Result of the last `filter` import, waiting for `host_read`.
    response: Vec<u8>,
    /// Resource limits applied to every instance.
    limits: StoreLimits,
}

impl HostData {
    pub(crate) fn new(
        sandbox: Sandbox,
        state: KernelState,
        dice: Dice,
        hexes: HashSet<HexId>,
    ) -> Self {
        Self {
            sandbox,
            state,
            dice,
            generation: 0,
            hexes,
            slots: Vec::new(),
            filters: HashMap::new(),
            read_only: false,
            stack: Vec::new(),
            response: Vec::new(),
            limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
        }
    }

    /// Store data with no game, used to read a plugin's manifest.
    fn detached(sandbox: Sandbox) -> Self {
        let state = KernelState {
            game_turn: 0,
            step_index: 0,
            status: GameStatus::Completed,
            units: BTreeMap::new(),
            city_control: BTreeMap::new(),
            pending_decision: None,
            rules: BTreeMap::new(),
        };
        let mut data = Self::new(sandbox, state, Dice::from_seed_text(""), HashSet::new());
        data.read_only = true;
        data
    }

    /// Marks every plugin mirror stale, after a rollback or a direct edit.
    pub(crate) fn invalidate(&mut self) {
        self.generation += 1;
    }

    /// Whether a plugin other than `slot` takes part in filter `name`.
    fn filter_active_for(&self, slot: usize, name: &str) -> bool {
        self.filters
            .get(name)
            .is_some_and(|providers| providers.iter().any(|provider| *provider != slot))
    }
}

pub(crate) fn new_store(data: HostData) -> Store<HostData> {
    let sandbox = data.sandbox;
    let mut store = Store::new(engine(sandbox), data);
    store.limiter(|data| &mut data.limits);
    if sandbox == Sandbox::Guarded {
        store.set_epoch_deadline(CALL_DEADLINE_TICKS);
    }
    store
}

/// The linker providing the [`imports`] in a sandbox.
fn linker(sandbox: Sandbox) -> &'static Linker<HostData> {
    static TRUSTED: OnceLock<Linker<HostData>> = OnceLock::new();
    static GUARDED: OnceLock<Linker<HostData>> = OnceLock::new();
    let slot = match sandbox {
        Sandbox::Trusted => &TRUSTED,
        Sandbox::Guarded => &GUARDED,
    };
    slot.get_or_init(|| {
        let mut linker = Linker::new(engine(sandbox));
        linker
            .func_wrap(
                imports::MODULE,
                imports::ROLL,
                |mut caller: Caller<'_, HostData>, sides: u32| -> wasmtime::Result<u32> {
                    let data = caller.data_mut();
                    if data.read_only {
                        return Err(wasmtime::format_err!(
                            "dice may not be rolled during a query or filter"
                        ));
                    }
                    if sides == 0 {
                        return Err(wasmtime::format_err!("a die needs at least one side"));
                    }
                    Ok(data.dice.roll(sides))
                },
            )
            .expect("unique import");
        linker
            .func_wrap(
                imports::MODULE,
                imports::FILTER_ACTIVE,
                |mut caller: Caller<'_, HostData>, ptr: u32, len: u32| -> wasmtime::Result<u32> {
                    let name = read_text(&mut caller, ptr, len)?;
                    let data = caller.data();
                    let slot = *data
                        .stack
                        .last()
                        .ok_or_else(|| wasmtime::format_err!("filter_active outside a call"))?;
                    Ok(u32::from(data.filter_active_for(slot, &name)))
                },
            )
            .expect("unique import");
        linker
            .func_wrap(
                imports::MODULE,
                imports::FILTER,
                |mut caller: Caller<'_, HostData>, ptr: u32, len: u32| -> wasmtime::Result<u32> {
                    let memory = caller_memory(&mut caller)?;
                    let mut bytes = vec![0; len as usize];
                    memory.read(&caller, ptr as usize, &mut bytes)?;
                    let result = abi::decode(&bytes).and_then(|(name, input, value, changes)| {
                        run_filter(&mut caller, name, input, value, changes)
                    });
                    let encoded = abi::encode(&result)?;
                    let len = encoded.len() as u32;
                    caller.data_mut().response = encoded;
                    Ok(len)
                },
            )
            .expect("unique import");
        linker
            .func_wrap(
                imports::MODULE,
                imports::HOST_READ,
                |mut caller: Caller<'_, HostData>, ptr: u32| -> wasmtime::Result<()> {
                    let memory = caller_memory(&mut caller)?;
                    let bytes = std::mem::take(&mut caller.data_mut().response);
                    memory.write(&mut caller, ptr as usize, &bytes)?;
                    Ok(())
                },
            )
            .expect("unique import");
        linker
            .func_wrap(
                imports::MODULE,
                imports::LOG,
                |mut caller: Caller<'_, HostData>, ptr: u32, len: u32| -> wasmtime::Result<()> {
                    let message = read_text(&mut caller, ptr, len)?;
                    let data = caller.data();
                    if let Some(&slot) = data.stack.last() {
                        eprintln!("[{}] {message}", data.slots[slot].plugin.manifest.id);
                    }
                    Ok(())
                },
            )
            .expect("unique import");
        linker
    })
}

fn caller_memory(caller: &mut Caller<'_, HostData>) -> wasmtime::Result<Memory> {
    caller
        .get_export(exports::MEMORY)
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmtime::format_err!("plugin exports no memory"))
}

/// Reads UTF-8 text a plugin passed to an import.
fn read_text(caller: &mut Caller<'_, HostData>, ptr: u32, len: u32) -> wasmtime::Result<String> {
    let memory = caller_memory(caller)?;
    let mut bytes = vec![0; len as usize];
    memory.read(&*caller, ptr as usize, &mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

/// Runs a filter for the calling plugin through every other plugin taking
/// part in it, after applying the caller's pending changes.
fn run_filter(
    caller: &mut Caller<'_, HostData>,
    name: String,
    input: Value,
    mut value: Value,
    changes: Vec<Change>,
) -> Result<Value, RuleError> {
    let Some(&slot) = caller.data().stack.last() else {
        return Err(RuleError::protocol("Filter outside a plugin call"));
    };
    if !changes.is_empty() {
        apply_changes(caller.data_mut(), slot, changes)?;
    }
    let providers: Vec<usize> = caller
        .data()
        .filters
        .get(&name)
        .into_iter()
        .flatten()
        .copied()
        .filter(|provider| *provider != slot)
        .collect();
    let was_read_only = std::mem::replace(&mut caller.data_mut().read_only, true);
    let mut result = Ok(());
    for provider in providers {
        match filter(&mut *caller, provider, &name, &input, &value) {
            Ok(next) => value = next,
            Err(error) => {
                result = Err(error);
                break;
            }
        }
    }
    caller.data_mut().read_only = was_read_only;
    result.map(|()| value)
}

/// Applies a plugin's changes and records that its mirror now matches the state.
fn apply_changes(data: &mut HostData, slot: usize, changes: Vec<Change>) -> Result<(), RuleError> {
    if changes.is_empty() {
        return Ok(());
    }
    if data.read_only {
        data.slots[slot].synced = None;
        return Err(RuleError::new(
            "readOnlyViolation",
            format!(
                "Plugin {} changed state during a query or filter",
                data.slots[slot].plugin.manifest.id
            ),
        ));
    }
    let result = changes
        .into_iter()
        .try_for_each(|change| data.state.apply(change, &data.hexes));
    data.generation += 1;
    if result.is_ok() {
        data.slots[slot].synced = Some(data.generation);
    } else {
        data.slots[slot].synced = None;
    }
    result
}

/// `RulesPlugin::create_game` on the plugin of `slot`.
pub(crate) fn create_game(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    game_id: &GameId,
    scenario: &ScenarioRequest,
) -> Result<ScenarioSetup, RuleError> {
    call(ctx, slot, Method::CreateGame, &(game_id, scenario))
}

/// `RulesPlugin::attach` on the plugin of `slot`.
pub(crate) fn attach(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    attachment: &Attachment,
) -> Result<(), RuleError> {
    call(ctx, slot, Method::Attach, attachment)
}

/// `RulesPlugin::apply_setup` on the plugin of `slot`; returns its events.
pub(crate) fn apply_setup(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    setup: &Value,
) -> Result<Vec<Value>, RuleError> {
    call(ctx, slot, Method::ApplySetup, setup)
}

/// `RulesPlugin::phase_started` or `phase_ending` on the plugin of `slot`;
/// returns its events.
pub(crate) fn phase_hook(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    phase: &PhaseDefinition,
    starting: bool,
) -> Result<Vec<Value>, RuleError> {
    let method = if starting {
        Method::PhaseStarted
    } else {
        Method::PhaseEnding
    };
    call(ctx, slot, method, phase)
}

/// `RulesPlugin::command` on the plugin of `slot`; returns its events.
pub(crate) fn command(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    command: &Value,
) -> Result<Vec<Value>, RuleError> {
    call(ctx, slot, Method::Command, command)
}

/// `RulesPlugin::query` on the plugin of `slot`.
pub(crate) fn query(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    name: &str,
    input: &Value,
) -> Result<Value, RuleError> {
    call(ctx, slot, Method::Query, &(name, input))
}

/// `RulesPlugin::filter` on the plugin of `slot`.
fn filter(
    ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    name: &str,
    input: &Value,
    value: &Value,
) -> Result<Value, RuleError> {
    call(ctx, slot, Method::Filter, &(name, input, value))
}

/// Calls one method of the plugin of `slot`, synchronising its mirror first if
/// needed, and applies the changes it reports. A rejection is the plugin's
/// error; no changes are applied.
///
/// # Errors
///
/// Returns the plugin's rejection; `pluginFault` if the plugin traps or
/// answers malformed data (the instance is then replaced before its next
/// call); `reentrantPluginCall` for a call into a plugin already handling one;
/// and the error of any change that fails the kernel's consistency checks.
fn call<A: serde::Serialize + ?Sized, R: DeserializeOwned>(
    mut ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    method: Method,
    args: &A,
) -> Result<R, RuleError> {
    let mut ctx = ctx.as_context_mut();
    if ctx.data().stack.contains(&slot) {
        return Err(RuleError::new(
            "reentrantPluginCall",
            format!(
                "Plugin {} is already handling a call",
                ctx.data().slots[slot].plugin.manifest.id
            ),
        ));
    }
    let args = abi::encode(args)?;
    let exports = ensure_instance(&mut ctx, slot)?;
    if ctx.data().stack.is_empty() && ctx.data().sandbox == Sandbox::Guarded {
        ctx.set_epoch_deadline(CALL_DEADLINE_TICKS);
    }
    ctx.data_mut().stack.push(slot);
    let result = run(&mut ctx, slot, &exports, method, &args);
    ctx.data_mut().stack.pop();
    result
}

/// The calls that make up one method call: turn, sync if stale, the method,
/// and its changes.
fn run<R: DeserializeOwned>(
    ctx: &mut StoreContextMut<'_, HostData>,
    slot: usize,
    exports: &Exports,
    method: Method,
    args: &[u8],
) -> Result<R, RuleError> {
    let stateless = ctx.data().slots[slot].plugin.manifest.stateless;

    let turn = ctx.data().state.turn();
    let completed = u32::from(turn.status == GameStatus::Completed);
    exports
        .set_turn
        .call(
            &mut *ctx,
            (u32::from(turn.game_turn), turn.step_index as u32, completed),
        )
        .map_err(|error| faulted(ctx, slot, error))?;

    let data = ctx.data();
    if !method.resets_mirror() && !stateless && data.slots[slot].synced != Some(data.generation) {
        let generation = data.generation;
        let state = abi::encode(&data.state.mirror())?;
        let synced: Result<(), RuleError> = call_method(ctx, slot, exports, &exports.sync, &state)?;
        if let Err(error) = synced {
            ctx.data_mut().slots[slot].synced = None;
            return Err(error);
        }
        ctx.data_mut().slots[slot].synced = Some(generation);
    }

    let result: Result<R, RuleError> =
        call_method(ctx, slot, exports, exports.method(method), args)?;
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            // A rejected call may leave the plugin's mirror half-changed.
            ctx.data_mut().slots[slot].synced = None;
            return Err(error);
        }
    };
    if method.resets_mirror() {
        ctx.data_mut().slots[slot].synced = None;
    } else if !stateless {
        let changes: Result<Vec<Change>, RuleError> = exports
            .take_changes
            .call(&mut *ctx, ())
            .and_then(|packed| read_result(ctx, exports, packed))
            .map_err(|error| faulted(ctx, slot, error))
            .and_then(|raw| decode_result(ctx, slot, &raw))?;
        match changes {
            Ok(changes) => apply_changes(ctx.data_mut(), slot, changes)?,
            Err(error) => {
                ctx.data_mut().slots[slot].synced = None;
                return Err(error);
            }
        }
    }
    Ok(value)
}

/// Calls an export that takes one encoded argument and decodes its result.
/// The outer error is a fault; the inner result is the plugin's answer.
fn call_method<R: DeserializeOwned>(
    ctx: &mut StoreContextMut<'_, HostData>,
    slot: usize,
    exports: &Exports,
    func: &MethodFunc,
    args: &[u8],
) -> Result<Result<R, RuleError>, RuleError> {
    let raw = invoke(ctx, exports, func, args).map_err(|error| faulted(ctx, slot, error))?;
    decode_result(ctx, slot, &raw)
}

/// Decodes a plugin's result, treating malformed bytes as a fault.
fn decode_result<R: DeserializeOwned>(
    ctx: &mut StoreContextMut<'_, HostData>,
    slot: usize,
    raw: &[u8],
) -> Result<Result<R, RuleError>, RuleError> {
    abi::decode(raw).map_err(|error| {
        faulted(
            ctx,
            slot,
            format!("sent a malformed result: {}", error.message),
        )
    })
}

/// Records that the plugin of `slot` faulted: its instance is replaced before
/// its next call.
fn faulted(
    ctx: &mut StoreContextMut<'_, HostData>,
    slot: usize,
    error: impl std::fmt::Display,
) -> RuleError {
    let slot = &mut ctx.data_mut().slots[slot];
    slot.exports = None;
    slot.synced = None;
    fault(&slot.plugin.manifest.id, error)
}

/// Instantiates the plugin of `slot` if it has no live instance, attaching the
/// fresh instance to the game when the slot was attached before.
fn ensure_instance(
    ctx: &mut StoreContextMut<'_, HostData>,
    slot: usize,
) -> Result<Exports, RuleError> {
    if let Some(exports) = &ctx.data().slots[slot].exports {
        return Ok(exports.clone());
    }
    let plugin = ctx.data().slots[slot].plugin.clone();
    let sandbox = ctx.data().sandbox;
    let module = plugin.module(sandbox)?;
    let instance = linker(sandbox)
        .instantiate(&mut *ctx, module)
        .map_err(|error| fault(&plugin.manifest.id, error))?;
    let exports = Exports::of(&instance, &mut *ctx)
        .map_err(|message| RuleError::new("pluginFault", message))?;
    {
        let slot = &mut ctx.data_mut().slots[slot];
        slot.exports = Some(exports.clone());
        slot.synced = None;
    }
    if let Some(attachment) = ctx.data().slots[slot].attach.clone() {
        attach(&mut *ctx, slot, &attachment)?;
    }
    Ok(exports)
}

/// Writes an argument into the plugin, calls `func`, and reads the result.
fn invoke(
    ctx: &mut StoreContextMut<'_, HostData>,
    exports: &Exports,
    func: &MethodFunc,
    args: &[u8],
) -> wasmtime::Result<Vec<u8>> {
    let len = u32::try_from(args.len())?;
    let ptr = exports.alloc.call(&mut *ctx, len)?;
    exports.memory.write(&mut *ctx, ptr as usize, args)?;
    let packed = func.call(&mut *ctx, (ptr, len))?;
    read_result(ctx, exports, packed)
}

/// Copies the result buffer `(ptr << 32) | len` out of the plugin and frees it.
fn read_result(
    ctx: &mut StoreContextMut<'_, HostData>,
    exports: &Exports,
    packed: u64,
) -> wasmtime::Result<Vec<u8>> {
    let (ptr, len) = ((packed >> 32) as u32, packed as u32);
    let mut result = vec![0; len as usize];
    exports.memory.read(&*ctx, ptr as usize, &mut result)?;
    exports.free.call(&mut *ctx, (ptr, len))?;
    Ok(result)
}

fn fault(plugin_id: &str, error: impl std::fmt::Display) -> RuleError {
    RuleError::new("pluginFault", format!("Plugin {plugin_id} failed: {error}"))
}
