#!/bin/bash

# --- Configuration ---
# The old, hardcoded monitor ID you want to replace.
OLD_MONITOR_ID="GSM-412NTZN5Y021"

# The name of your dconf dump file.
DCONF_FILE="dash-to-panel-linexin.ini"

# The path to the Python helper script. Assumes it's in the same directory.
PYTHON_SCRIPT="./get_primary_monitor.py"
# --- End of Configuration ---


# 1. Validate that the necessary files exist.
if [ ! -f "$PYTHON_SCRIPT" ]; then
    echo "Error: Python script not found at '$PYTHON_SCRIPT'"
    exit 1
fi

if [ ! -f "$DCONF_FILE" ]; then
    echo "Error: Dconf file not found at '$DCONF_FILE'"
    exit 1
fi


# 2. Run the Python script to get the current primary monitor's ID.
echo "🔍 Finding current primary monitor..."
NEW_MONITOR_ID=$($PYTHON_SCRIPT)

# 3. Check if the Python script succeeded.
if [ -z "$NEW_MONITOR_ID" ]; then
    echo "❌ Error: Failed to get the new monitor ID from the Python script. Aborting."
    exit 1
fi

echo "✅ Current primary monitor is: $NEW_MONITOR_ID"


# 4. Compare the old and new IDs.
if [ "$OLD_MONITOR_ID" == "$NEW_MONITOR_ID" ]; then
    echo "👍 The monitor ID in the file is already correct. No changes needed."
    exit 0
fi


# 5. Perform the replacement using sed.
echo "🔄 Replacing all instances of '$OLD_MONITOR_ID' with '$NEW_MONITOR_ID' in '$DCONF_FILE'..."

# The 'sed -i' command modifies the file in-place.
# The "s/find/replace/g" pattern with double quotes allows shell variables to be used.
sed -i "s/$OLD_MONITOR_ID/$NEW_MONITOR_ID/g" "$DCONF_FILE"

echo "🎉 Done! The file '$DCONF_FILE' has been updated."
