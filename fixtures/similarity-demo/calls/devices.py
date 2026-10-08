def retire_device(registry, device_id, note):
    device = registry.devices.get(device_id)
    if device is None:
        raise LookupError(device_id)
    registry.network.revoke(device.address)
    registry.audit.write("retire", device_id, note)
    registry.devices.remove(device_id)
    return device
