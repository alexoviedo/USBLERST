//! Host-compilable firmware bootstrap for the USB-to-BLE bridge workspace.

mod app;

pub use app::App;

fn main() {
    usb2ble_platform_espidf::link_patches_if_needed();

    let bootstrap_result = app::bootstrap_default();

    match bootstrap_result {
        Ok(app_instance) => {
            println!(
                "bootstrap: app={}, core={}, proto={}, platform={}, profile={}",
                app::APP_NAME,
                usb2ble_core::CORE_CRATE_NAME,
                usb2ble_proto::PROTO_CRATE_NAME,
                usb2ble_platform_espidf::PLATFORM_CRATE_NAME,
                app_instance.runtime().active_profile().as_str()
            );
        }
        Err(error) => {
            println!("bootstrap failed: {:?}", error);
        }
    }
}
