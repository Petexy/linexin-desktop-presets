#!/usr/bin/python3
# -*- coding: utf-8 -*-
# This script replicates the monitor discovery logic from Dash-to-Panel's panelSettings.js

import sys
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio

def get_primary_monitor_id():
    """
    Finds the primary monitor's ID using the same logic as the Dash-to-Panel extension:
    1. Calls Mutter's GetCurrentState D-Bus method.
    2. Identifies the primary monitor via its 'primary' flag.
    3. Constructs the ID from the monitor's vendor and serial number ('vendor-serial').
    """
    try:
        # Connect to the D-Bus interface for display configuration
        proxy = Gio.DBusProxy.new_for_bus_sync(
            Gio.BusType.SESSION,
            Gio.DBusProxyFlags.NONE,
            None,
            'org.gnome.Mutter.DisplayConfig',
            '/org/gnome/Mutter/DisplayConfig',
            'org.gnome.Mutter.DisplayConfig',
            None
        )

        # Call the GetCurrentState method, just like the extension does
        variant = proxy.call_sync('GetCurrentState', None, Gio.DBusCallFlags.NONE, -1, None)
        resources = variant.unpack()
        
        # The logical monitors are the third element (index 2) in the result
        logical_monitors = resources[2]

        for monitor in logical_monitors:
            # The 'is_primary' flag is the fifth element (index 4)
            is_primary = monitor[4]
            
            if is_primary:
                # The physical monitor details are in the sixth element (index 5)
                # We get vendor and serial from the first physical monitor attached.
                # Corresponds to: let [c, vendor, product, serial] = logicalMonitor[5][0]
                details = monitor[5][0]
                vendor = details[1]
                serial = details[3]
                
                # Replicate the ID creation: if (vendor && serial) id = `${vendor}-${serial}`
                if vendor and serial:
                    return f"{vendor}-{serial}"
                else:
                    # Fallback if vendor or serial is missing (unlikely)
                    return details[0] # connector name

    except Exception as e:
        print(f"An error occurred: {e}", file=sys.stderr)
        return None

    return None

if __name__ == "__main__":
    primary_monitor_id = get_primary_monitor_id()
    if primary_monitor_id:
        print(primary_monitor_id)
    else:
        print("Could not determine primary monitor ID.", file=sys.stderr)
        sys.exit(1)