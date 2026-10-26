use martini::erase::device::{DeviceTarget, resolve_device_target};

#[test]
fn test_resolve_device_target_parsing() {
    assert_eq!(resolve_device_target("auto").unwrap(), DeviceTarget::Auto);
    assert_eq!(resolve_device_target("cuda").unwrap(), DeviceTarget::Cuda);
    assert_eq!(resolve_device_target("cpu").unwrap(), DeviceTarget::Cpu);
    assert!(resolve_device_target("invalid").is_err());
}
