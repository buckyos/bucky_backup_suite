import os
import platform
import shutil
import sys
import tempfile
import time

src_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
root_bin_dir = os.path.join(src_dir, "rootfs/bin")

def strip_and_copy_rust_file(rust_target_dir, name, dest, need_dir=False):
    src_file = os.path.join(rust_target_dir, "release", name)
    if need_dir:
        dest = os.path.join(dest, name)
        os.makedirs(dest, exist_ok=True)
        #sleep 0.1s
        #time.sleep(0.1)
    #print(f"copying {src_file} to {dest}")
    #os.system(f"cp {src_file} {dest}")
    if platform.system() == "Windows":
        src_file = src_file + ".exe"
    shutil.copy(src_file, dest)
    print(f"stripping {os.path.join(dest, name)}")
    os.system(f"strip {os.path.join(dest, name)}")

def copy_web_apps(src, target):
    dist_dir = os.path.join(src_dir, src, "dist")
    os.makedirs(target, exist_ok=True)
    print(f'copying vite build {dist_dir} to {target}')
    shutil.rmtree(target)
    shutil.copytree(dist_dir, target, copy_function=shutil.copyfile)
    pass

def copy_files(rust_target_dir):
    print("Copying files...")
    # code to copy files
    bin = "backup_suite"
    strip_and_copy_rust_file(rust_target_dir, bin, root_bin_dir, True)
    copy_web_apps("webui/src", os.path.join(root_bin_dir, "backup_suite","webui"))

    print("Files copied successfully!")

if __name__ == "__main__":
    args = sys.argv[1:]
    print("MUST RUN build.py FIRST!!")
    if len(args) == 0:
        print("NEED ARGUMENT: amd64|aarch64")
        exit(1)
    if len(args) > 0:
        temp_dir = tempfile.gettempdir()
        project_name = "buckyos"
        target_dir = os.path.join(temp_dir, "rust_build", project_name)
        if args[0] == "amd64":
            copy_files(os.path.join(target_dir, "x86_64-unknown-linux-musl"))
        elif args[0] == "aarch64":
            copy_files(os.path.join(target_dir, "aarch64-unknown-linux-musl"))
        else:
            print("Invalid argument: clean|amd64|aarch64")
            exit(1)