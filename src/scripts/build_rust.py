import os
import tempfile
import sys
import subprocess

src_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

def clean(target_dir):
    print(f"Cleaning build artifacts at ${target_dir}")
    subprocess.run(["cargo", "clean", "--target-dir", target_dir], check=True, cwd=src_dir)

def build_rust(target_dir, target):
    print("Building Rust code")
    env = os.environ.copy()
    env["OPENSSL_STATIC"] = "1"
    env["RUSTFLAGS"] = "-C target-feature=+crt-static --cfg tokio_unstable"
    subprocess.run(["cargo", "build", "--target", target, "--release", "--target-dir", target_dir], 
                   check=True, 
                   cwd=src_dir, 
                   env=env)

if __name__ == "__main__":
    args = sys.argv[1:]
    if len(args) == 0:
        print("NEED ARGUMENT: clean|amd64|aarch64")
        exit(1)
    if len(args) > 0:
        temp_dir = tempfile.gettempdir()
        project_name = "backup_suite"
        target_dir = os.path.join(temp_dir, "rust_build", project_name)
        os.makedirs(target_dir, exist_ok=True)
        if args[0] == "clean":
            clean(target_dir)
        elif args[0] == "amd64":
            build_rust(target_dir, "x86_64-unknown-linux-musl")
        elif args[0] == "aarch64":
            build_rust(target_dir, "aarch64-unknown-linux-gnu")
        else:
            print("Invalid argument: clean|amd64|aarch64")
            exit(1)