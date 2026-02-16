isDebugOn = True
win_version = ""


def user_input():
    global isDebugOn, win_version
    while True:
        cmd = input("Enter command (debug on/off, version <Window 10 Build Number(normal: 19041)>, exit): ").strip().lower()
        if cmd == "debug on":
            isDebugOn = True
            print("Debug mode enabled.")
        elif cmd == "debug off":
            isDebugOn = False
            print("Debug mode disabled.")
        elif cmd.startswith("version "):
            _, version = cmd.split(maxsplit=1)
            if version in ["win10", "win11"]:
                win_version = version
                print(f"Windows version set to {win_version}.")
            else:
                print("Invalid version. Use correct build number.")
        elif cmd == "exit":
            print("Exiting command interface.")
            break
        else:
            print("Unknown command. Please try again.")