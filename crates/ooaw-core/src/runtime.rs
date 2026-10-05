//! WebAssembly plugin runtime.
//!
//! Plugins are compiled once per process with Cranelift and instantiated once
//! per game. All instances of a game share one [`Store`], whose data holds the
//! kernel's authoritative state, so a host import running for one plugin can
//! apply its changes and call the other plugins taking part in a filter.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Once, OnceLock};
use std::time::Duration;

use ooaw_plugin_api::protocol::{
    DispatchRequest, DispatchResponse, HostRequest, HostResponse, PluginCall, PluginManifest,
    ABI_VERSION,
};
use ooaw_plugin_api::serde_json::{self, Value};
use ooaw_plugin_api::{GameStatus, HexId, RuleError};
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
        .get_typed_func::<(), u32>(&mut store, "ooaw_abi_version")
        .and_then(|func| func.call(&mut store, ()))
        .map_err(|error| load_error(format!("Plugin exports no ABI version: {error}")))?;
    if version != ABI_VERSION {
        return Err(load_error(format!(
            "Plugin uses ABI version {version}, but the kernel supports {ABI_VERSION}"
        )));
    }
    let exports = Exports::of(&instance, &mut store).map_err(load_error)?;
    let request: DispatchRequest = DispatchRequest {
        turn: None,
        sync: None,
        active_filters: Vec::new(),
        call: PluginCall::Manifest,
    };
    let bytes = serde_json::to_vec(&request).map_err(|error| load_error(error.to_string()))?;
    let raw = invoke(&mut store.as_context_mut(), &exports, &bytes)
        .map_err(|error| load_error(format!("Plugin failed to describe itself: {error}")))?;
    let response: DispatchResponse = serde_json::from_slice(&raw)
        .map_err(|error| load_error(format!("Malformed manifest response: {error}")))?;
    let manifest: PluginManifest = serde_json::from_value(response.result?)
        .map_err(|error| load_error(format!("Malformed manifest: {error}")))?;
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

/// Exports of one plugin instance.
#[derive(Clone)]
struct Exports {
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    free: TypedFunc<(u32, u32), ()>,
    dispatch: TypedFunc<(u32, u32), u64>,
}

impl Exports {
    fn of(instance: &wasmtime::Instance, mut store: impl AsContextMut) -> Result<Self, String> {
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or("Plugin exports no memory")?;
        let alloc = instance
            .get_typed_func(&mut store, "ooaw_alloc")
            .map_err(|error| error.to_string())?;
        let free = instance
            .get_typed_func(&mut store, "ooaw_free")
            .map_err(|error| error.to_string())?;
        let dispatch = instance
            .get_typed_func(&mut store, "ooaw_dispatch")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            memory,
            alloc,
            free,
            dispatch,
        })
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
    /// Call that attaches a fresh instance to the game.
    pub(crate) attach: Option<PluginCall>,
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
    /// Response to the last `host_call`, waiting for `host_read`.
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

    /// Filters some plugin other than `slot` takes part in.
    fn active_filters_for(&self, slot: usize) -> Vec<String> {
        let mut names: Vec<String> = self
            .filters
            .iter()
            .filter(|(_, providers)| providers.iter().any(|provider| *provider != slot))
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        names
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

/// The linker providing the `ooaw` imports in a sandbox.
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
                "ooaw",
                "roll",
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
                "ooaw",
                "host_call",
                |mut caller: Caller<'_, HostData>, ptr: u32, len: u32| -> wasmtime::Result<u32> {
                    let memory = caller_memory(&mut caller)?;
                    let mut bytes = vec![0; len as usize];
                    memory.read(&caller, ptr as usize, &mut bytes)?;
                    let response: HostResponse = match serde_json::from_slice(&bytes) {
                        Ok(request) => host_request(&mut caller, request),
                        Err(error) => Err(RuleError::protocol(format!(
                            "Malformed host request: {error}"
                        ))),
                    };
                    let encoded = serde_json::to_vec(&response)?;
                    let len = encoded.len() as u32;
                    caller.data_mut().response = encoded;
                    Ok(len)
                },
            )
            .expect("unique import");
        linker
            .func_wrap(
                "ooaw",
                "host_read",
                |mut caller: Caller<'_, HostData>, ptr: u32| -> wasmtime::Result<()> {
                    let memory = caller_memory(&mut caller)?;
                    let bytes = std::mem::take(&mut caller.data_mut().response);
                    memory.write(&mut caller, ptr as usize, &bytes)?;
                    Ok(())
                },
            )
            .expect("unique import");
        linker
    })
}

fn caller_memory(caller: &mut Caller<'_, HostData>) -> wasmtime::Result<Memory> {
    caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| wasmtime::format_err!("plugin exports no memory"))
}

/// Handles one plugin-to-kernel request.
fn host_request(caller: &mut Caller<'_, HostData>, request: HostRequest) -> HostResponse {
    let Some(&slot) = caller.data().stack.last() else {
        return Err(RuleError::protocol("Host request outside a plugin call"));
    };
    match request {
        HostRequest::Log { message } => {
            eprintln!(
                "[{}] {message}",
                caller.data().slots[slot].plugin.manifest.id
            );
            Ok(Value::Null)
        }
        HostRequest::Filter {
            name,
            input,
            mut value,
            changes,
        } => {
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
                let call = PluginCall::Filter {
                    name: name.clone(),
                    input: input.clone(),
                    value,
                };
                match dispatch(&mut *caller, provider, call) {
                    Ok(response) => match response.result {
                        Ok(next) => value = next,
                        Err(error) => {
                            result = Err(error);
                            value = Value::Null;
                            break;
                        }
                    },
                    Err(error) => {
                        result = Err(error);
                        value = Value::Null;
                        break;
                    }
                }
            }
            caller.data_mut().read_only = was_read_only;
            result.map(|()| value)
        }
    }
}

/// Applies a plugin's changes and records that its mirror now matches the state.
fn apply_changes(
    data: &mut HostData,
    slot: usize,
    changes: Vec<ooaw_plugin_api::protocol::Change>,
) -> Result<(), RuleError> {
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

/// Sends one call to a plugin, synchronising its mirror first if needed, and
/// applies the changes it reports. Returns the plugin's response; a rejected
/// call is `Ok` with an `Err` result and no changes applied.
///
/// # Errors
///
/// Returns `pluginFault` if the plugin traps or answers malformed data (the
/// instance is then replaced before its next call), `reentrantPluginCall` for
/// a call into a plugin already handling one, and the error of any change
/// that fails the kernel's consistency checks.
pub(crate) fn dispatch(
    mut ctx: impl AsContextMut<Data = HostData>,
    slot: usize,
    call: PluginCall,
) -> Result<DispatchResponse, RuleError> {
    let mut ctx = ctx.as_context_mut();
    let plugin_id = ctx.data().slots[slot].plugin.manifest.id.clone();
    if ctx.data().stack.contains(&slot) {
        return Err(RuleError::new(
            "reentrantPluginCall",
            format!("Plugin {plugin_id} is already handling a call"),
        ));
    }
    let exports = ensure_instance(&mut ctx, slot)?;
    if ctx.data().stack.is_empty() && ctx.data().sandbox == Sandbox::Guarded {
        ctx.set_epoch_deadline(CALL_DEADLINE_TICKS);
    }
    let resets_mirror = matches!(
        call,
        PluginCall::Manifest | PluginCall::CreateGame { .. } | PluginCall::Attach { .. }
    );
    let data = ctx.data();
    let stale = data.slots[slot].synced != Some(data.generation)
        && !data.slots[slot].plugin.manifest.stateless;
    let request = DispatchRequest {
        turn: Some(data.state.turn()),
        sync: (stale && !resets_mirror).then(|| data.state.mirror()),
        active_filters: data.active_filters_for(slot),
        call,
    };
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| RuleError::protocol(format!("Unserializable request: {error}")))?;
    if request.sync.is_some() {
        let generation = ctx.data().generation;
        ctx.data_mut().slots[slot].synced = Some(generation);
    }

    ctx.data_mut().stack.push(slot);
    let raw = invoke(&mut ctx, &exports, &bytes);
    ctx.data_mut().stack.pop();
    let response = raw
        .map_err(|error| fault(&plugin_id, error))
        .and_then(|raw| {
            serde_json::from_slice::<DispatchResponse>(&raw).map_err(|error| {
                RuleError::new(
                    "pluginFault",
                    format!("Plugin {plugin_id} sent a malformed response: {error}"),
                )
            })
        });
    let mut response = match response {
        Ok(response) => response,
        Err(error) => {
            let slot = &mut ctx.data_mut().slots[slot];
            slot.exports = None;
            slot.synced = None;
            return Err(error);
        }
    };

    let data = ctx.data_mut();
    if resets_mirror {
        data.slots[slot].synced = None;
    } else if response.result.is_err() {
        // A rejected call may leave the plugin's mirror half-changed.
        data.slots[slot].synced = None;
    } else {
        let changes = std::mem::take(&mut response.changes);
        apply_changes(data, slot, changes)?;
    }
    Ok(response)
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
    if let Some(attach) = ctx.data().slots[slot].attach.clone() {
        let response = dispatch(&mut *ctx, slot, attach)?;
        response.result?;
    }
    Ok(exports)
}

/// Writes a request into the plugin, runs `ooaw_dispatch`, and reads the response.
fn invoke(
    ctx: &mut StoreContextMut<'_, HostData>,
    exports: &Exports,
    request: &[u8],
) -> wasmtime::Result<Vec<u8>> {
    let len = u32::try_from(request.len())?;
    let ptr = exports.alloc.call(&mut *ctx, len)?;
    exports.memory.write(&mut *ctx, ptr as usize, request)?;
    let packed = exports.dispatch.call(&mut *ctx, (ptr, len))?;
    let (out_ptr, out_len) = ((packed >> 32) as u32, packed as u32);
    let mut response = vec![0; out_len as usize];
    exports
        .memory
        .read(&*ctx, out_ptr as usize, &mut response)?;
    exports.free.call(&mut *ctx, (out_ptr, out_len))?;
    Ok(response)
}

fn fault(plugin_id: &str, error: impl std::fmt::Display) -> RuleError {
    RuleError::new("pluginFault", format!("Plugin {plugin_id} failed: {error}"))
}
