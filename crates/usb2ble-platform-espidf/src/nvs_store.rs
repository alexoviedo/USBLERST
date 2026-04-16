/// NVS namespace used for persisted usb2ble settings.
pub const PROFILE_NAMESPACE: &str = "usb2ble";

/// NVS key used to store the active profile selection.
pub const ACTIVE_PROFILE_KEY: &str = "active_profile";

/// NVS key used to store the bond presence state.
pub const BONDS_PRESENT_KEY: &str = "bonds_present";

/// Errors that can occur when using the storage boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    /// The storage backend is unavailable.
    Unavailable,
    /// The storage backend reported a failure.
    BackendFailure,
}

/// Profile persistence boundary for the future ESP-IDF glue.
pub trait ProfileStore {
    /// Loads the active profile if one is persisted.
    fn load_active_profile(&self) -> Option<usb2ble_core::profile::ProfileId>;

    /// Persists the active profile.
    fn store_active_profile(
        &mut self,
        profile: usb2ble_core::profile::ProfileId,
    ) -> Result<(), StoreError>;
}

/// ESP-IDF-backed profile storage adapter for embedded builds.
#[cfg(target_os = "espidf")]
pub struct EspNvsProfileStore {
    nvs: esp_idf_svc::nvs::EspNvs<esp_idf_svc::nvs::NvsDefault>,
}

/// Host stub for the ESP-IDF-backed profile storage adapter.
#[cfg(not(target_os = "espidf"))]
pub struct EspNvsProfileStore;

#[cfg(target_os = "espidf")]
impl EspNvsProfileStore {
    /// Opens the default NVS partition and the usb2ble profile namespace.
    pub fn new() -> Result<Self, StoreError> {
        let partition = esp_idf_svc::nvs::EspNvsPartition::<esp_idf_svc::nvs::NvsDefault>::take()
            .map_err(|_| StoreError::BackendFailure)?;
        let nvs = esp_idf_svc::nvs::EspNvs::new(partition, PROFILE_NAMESPACE, true)
            .map_err(|_| StoreError::BackendFailure)?;

        Ok(Self { nvs })
    }
}

#[cfg(not(target_os = "espidf"))]
impl EspNvsProfileStore {
    /// Creates the host stub, which remains unavailable outside ESP-IDF builds.
    pub fn new() -> Result<Self, StoreError> {
        Err(StoreError::Unavailable)
    }
}

#[cfg(target_os = "espidf")]
fn profile_from_raw_tag(raw: u8) -> Option<usb2ble_core::profile::ProfileId> {
    match raw {
        1 => Some(usb2ble_core::profile::V1_PROFILE_ID),
        _ => None,
    }
}

#[cfg(target_os = "espidf")]
fn raw_tag_from_profile(profile: usb2ble_core::profile::ProfileId) -> u8 {
    match profile {
        usb2ble_core::profile::ProfileId::T16000mV1 => 1,
    }
}

#[cfg(target_os = "espidf")]
impl ProfileStore for EspNvsProfileStore {
    fn load_active_profile(&self) -> Option<usb2ble_core::profile::ProfileId> {
        let mut buf = [0_u8; 1];

        match self.nvs.get_raw(ACTIVE_PROFILE_KEY, &mut buf) {
            Ok(Some(stored)) if !stored.is_empty() => profile_from_raw_tag(stored[0]),
            Ok(Some(_)) | Ok(None) | Err(_) => None,
        }
    }

    fn store_active_profile(
        &mut self,
        profile: usb2ble_core::profile::ProfileId,
    ) -> Result<(), StoreError> {
        let encoded = [raw_tag_from_profile(profile)];

        self.nvs
            .set_raw(ACTIVE_PROFILE_KEY, &encoded)
            .map(|_| ())
            .map_err(|_| StoreError::BackendFailure)
    }
}

#[cfg(not(target_os = "espidf"))]
impl ProfileStore for EspNvsProfileStore {
    fn load_active_profile(&self) -> Option<usb2ble_core::profile::ProfileId> {
        None
    }

    fn store_active_profile(
        &mut self,
        _profile: usb2ble_core::profile::ProfileId,
    ) -> Result<(), StoreError> {
        Err(StoreError::Unavailable)
    }
}

/// ESP-IDF-backed bond storage adapter for embedded builds.
#[cfg(target_os = "espidf")]
pub struct EspNvsBondStore {
    nvs: esp_idf_svc::nvs::EspNvs<esp_idf_svc::nvs::NvsDefault>,
}

/// Host stub for the ESP-IDF-backed bond storage adapter.
#[cfg(not(target_os = "espidf"))]
pub struct EspNvsBondStore;

#[cfg(target_os = "espidf")]
impl EspNvsBondStore {
    /// Opens the default NVS partition and the usb2ble profile namespace.
    pub fn new() -> Result<Self, StoreError> {
        let partition = esp_idf_svc::nvs::EspNvsPartition::<esp_idf_svc::nvs::NvsDefault>::take()
            .map_err(|_| StoreError::BackendFailure)?;
        let nvs = esp_idf_svc::nvs::EspNvs::new(partition, PROFILE_NAMESPACE, true)
            .map_err(|_| StoreError::BackendFailure)?;

        Ok(Self { nvs })
    }
}

#[cfg(not(target_os = "espidf"))]
impl EspNvsBondStore {
    /// Creates the host stub, which remains unavailable outside ESP-IDF builds.
    pub fn new() -> Result<Self, StoreError> {
        Err(StoreError::Unavailable)
    }
}

#[cfg(target_os = "espidf")]
impl BondStore for EspNvsBondStore {
    fn bonds_present(&self) -> bool {
        let mut buf = [0_u8; 1];

        match self.nvs.get_raw(BONDS_PRESENT_KEY, &mut buf) {
            Ok(Some(stored)) if !stored.is_empty() => stored[0] != 0,
            Ok(Some(_)) | Ok(None) | Err(_) => false,
        }
    }

    fn store_bonds_present(&mut self, bonds_present: bool) -> Result<(), StoreError> {
        let encoded = [if bonds_present { 1_u8 } else { 0_u8 }];

        self.nvs
            .set_raw(BONDS_PRESENT_KEY, &encoded)
            .map(|_| ())
            .map_err(|_| StoreError::BackendFailure)
    }

    fn clear_bonds(&mut self) -> Result<(), StoreError> {
        self.store_bonds_present(false)
    }
}

#[cfg(not(target_os = "espidf"))]
impl BondStore for EspNvsBondStore {
    fn bonds_present(&self) -> bool {
        false
    }

    fn store_bonds_present(&mut self, _bonds_present: bool) -> Result<(), StoreError> {
        Err(StoreError::Unavailable)
    }

    fn clear_bonds(&mut self) -> Result<(), StoreError> {
        Err(StoreError::Unavailable)
    }
}

/// Bond storage boundary for the future ESP-IDF glue.
pub trait BondStore {
    /// Returns whether any persisted bonds are present.
    fn bonds_present(&self) -> bool;

    /// Persists whether bonds are present.
    fn store_bonds_present(&mut self, bonds_present: bool) -> Result<(), StoreError>;

    /// Clears all persisted bonds.
    fn clear_bonds(&mut self) -> Result<(), StoreError>;
}

/// In-memory profile storage adapter for host-side use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryProfileStore {
    active_profile: Option<usb2ble_core::profile::ProfileId>,
}

impl Default for MemoryProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryProfileStore {
    /// Creates an empty in-memory profile store.
    pub fn new() -> Self {
        Self {
            active_profile: None,
        }
    }

    /// Creates an in-memory profile store with an active profile.
    pub fn with_profile(profile: usb2ble_core::profile::ProfileId) -> Self {
        Self {
            active_profile: Some(profile),
        }
    }

    /// Returns the currently stored active profile, if any.
    pub fn active_profile(&self) -> Option<usb2ble_core::profile::ProfileId> {
        self.active_profile
    }
}

impl ProfileStore for MemoryProfileStore {
    fn load_active_profile(&self) -> Option<usb2ble_core::profile::ProfileId> {
        self.active_profile
    }

    fn store_active_profile(
        &mut self,
        profile: usb2ble_core::profile::ProfileId,
    ) -> Result<(), StoreError> {
        self.active_profile = Some(profile);
        Ok(())
    }
}

/// In-memory bond storage adapter for host-side use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBondStore {
    bonds_present: bool,
}

impl Default for MemoryBondStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryBondStore {
    /// Creates an in-memory bond store with no bonds.
    pub fn new() -> Self {
        Self {
            bonds_present: false,
        }
    }

    /// Creates an in-memory bond store with the provided state.
    pub fn with_bonds_present(bonds_present: bool) -> Self {
        Self { bonds_present }
    }

    /// Sets whether bonds are present.
    pub fn set_bonds_present(&mut self, bonds_present: bool) {
        let _ = self.store_bonds_present(bonds_present);
    }
}

impl BondStore for MemoryBondStore {
    fn bonds_present(&self) -> bool {
        self.bonds_present
    }

    fn store_bonds_present(&mut self, bonds_present: bool) -> Result<(), StoreError> {
        self.bonds_present = bonds_present;
        Ok(())
    }

    fn clear_bonds(&mut self) -> Result<(), StoreError> {
        self.store_bonds_present(false)
    }
}

#[cfg(all(test, not(target_os = "espidf")))]
mod tests {
    use super::{BondStore, EspNvsBondStore, EspNvsProfileStore, ProfileStore, StoreError};
    use usb2ble_core::profile::V1_PROFILE_ID;

    #[test]
    fn esp_nvs_profile_store_new_is_unavailable_on_host() {
        assert!(matches!(
            EspNvsProfileStore::new(),
            Err(StoreError::Unavailable)
        ));
    }

    #[test]
    fn esp_nvs_profile_store_load_returns_none_on_host() {
        let store = EspNvsProfileStore;

        assert_eq!(store.load_active_profile(), None);
    }

    #[test]
    fn esp_nvs_profile_store_store_returns_unavailable_on_host() {
        let mut store = EspNvsProfileStore;

        assert_eq!(
            store.store_active_profile(V1_PROFILE_ID),
            Err(StoreError::Unavailable)
        );
    }

    #[test]
    fn esp_nvs_bond_store_new_is_unavailable_on_host() {
        assert!(matches!(
            EspNvsBondStore::new(),
            Err(StoreError::Unavailable)
        ));
    }

    #[test]
    fn esp_nvs_bond_store_bonds_present_returns_false_on_host() {
        let store = EspNvsBondStore;

        assert!(!store.bonds_present());
    }

    #[test]
    fn esp_nvs_bond_store_store_returns_unavailable_on_host() {
        let mut store = EspNvsBondStore;

        assert_eq!(
            store.store_bonds_present(true),
            Err(StoreError::Unavailable)
        );
    }

    #[test]
    fn esp_nvs_bond_store_clear_returns_unavailable_on_host() {
        let mut store = EspNvsBondStore;

        assert_eq!(store.clear_bonds(), Err(StoreError::Unavailable));
    }
}
