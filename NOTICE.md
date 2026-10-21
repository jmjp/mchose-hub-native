# NOTICE

This project is an independent, clean-room interoperability effort. It is not
affiliated with, endorsed by, or sponsored by MCHOSE or C-Media.

- "MCHOSE", "M HUB", "V9 PRO" and related marks belong to their respective owners.
- No vendor binaries (Electron app, `osConfLib.dll`, driver packages, firmware images)
  are redistributed in this repository.
- The vendor DSP integration calls the locally installed `osConfLib.dll` at runtime;
  it is not bundled. Driver install/repair reuses the INF packages installed by the
  official application.
- Device protocol constants were recovered for interoperability from the shipped
  application and the device itself.
- The product illustration in `assets/product_v9pro.png` is an original placeholder
  drawing created for this project (not a vendor asset).
- Factory preset values are numeric configuration data recovered from the device's
  local preset store.
