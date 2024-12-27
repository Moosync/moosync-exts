import os
import json

def find_and_consolidate_package_json():
    # Get the current directory
    current_dir = os.getcwd()

    # Dictionary to store consolidated data
    consolidated_data = {}

    # Traverse through all directories in the current directory, depth = 1
    for root, dirs, files in os.walk(current_dir):
        # Limit depth to 1 by filtering subdirectories
        if root != current_dir:
            del dirs[:]

        if 'package.json' in files:
            package_json_path = os.path.join(root, 'package.json')
            print(f"Found package.json at: {package_json_path}")

            # Parse the package.json file
            try:
                with open(package_json_path, 'r', encoding='utf-8') as file:
                    package_data = json.load(file)

                    # Extract required fields
                    name = package_data.get('name')
                    if name:
                        # Resolve the icon path relative to current_dir if it exists
                        icon = package_data.get("icon")
                        if icon:
                            icon = os.path.relpath(os.path.join(root, icon), current_dir)

                        consolidated_data[name] = {
                            "displayName": package_data.get("displayName"),
                            "version": package_data.get("version"),
                            "icon": icon,
                            "permissions": package_data.get("permissions", [])
                        }

            except json.JSONDecodeError as e:
                print(f"Error decoding JSON from {package_json_path}: {e}")
            except Exception as e:
                print(f"Error reading {package_json_path}: {e}")

    # Create the output directory
    output_dir = os.path.join(current_dir, 'dist')
    os.makedirs(output_dir, exist_ok=True)

    # Output the consolidated data to a JSON file
    output_file = os.path.join(output_dir, 'manifest.json')
    try:
        with open(output_file, 'w', encoding='utf-8') as outfile:
            json.dump(consolidated_data, outfile, indent=4)
        print(f"Consolidated data written to {output_file}")
    except Exception as e:
        print(f"Error writing consolidated data to file: {e}")

if __name__ == "__main__":
    find_and_consolidate_package_json()
