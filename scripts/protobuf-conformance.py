#!/usr/bin/env python3
"""Execute owned SDKs against an independently generated, pinned gRPC peer.

All servers bind loopback on ephemeral ports. Upstream generation happens only
inside this test harness; production packages are generated through Poolster's SDK.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
PEERS = ROOT / "scripts/protobuf-conformance"
CORPUS = ROOT / "crates/inputs/protobuf/tests/fixtures/conformance"


def run(command, cwd, env=None, timeout=120):
    result = subprocess.run(command, cwd=cwd, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}): {command}\n{result.stdout}")
    return result.stdout


def ready(process, path):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if path.exists():
            address = path.read_text().strip()
            if address:
                return address
        if process.poll() is not None:
            raise RuntimeError(f"Server exited before readiness: {process.returncode}")
        time.sleep(0.05)
    raise TimeoutError("Local conformance server did not become ready")


def execute_pair(server, client, directory, env_server, env_client, label):
    signal = directory / f"{label}.ready"
    log = directory / f"{label}.log"
    started = time.monotonic()
    with log.open("w") as output:
        process = subprocess.Popen(server + ["server", str(signal)],
                                   cwd=directory, env=env_server,
                                   stdout=output, stderr=subprocess.STDOUT)
        try:
            address = ready(process, signal)
            run(client + ["client", address], directory, env_client, timeout=30)
        except Exception as error:
            raise RuntimeError(f"{label}: {error}\n{log.read_text()}") from error
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    return {"case": label, "status": "passed", "seconds": round(time.monotonic()-started, 3)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True,
                        help="Evidence JSON path; generated packages use a temporary directory")
    parser.add_argument("--oracle-python", default=os.environ.get("POOLSTER_PROTOBUF_ORACLE_PYTHON"))
    parser.add_argument("--python", default=os.environ.get("POOLSTER_PYTHON_GRPC"))
    parser.add_argument("--typescript-tools", default=os.environ.get("POOLSTER_TS_GRPC_TOOLS"))
    args = parser.parse_args()
    if not all([args.oracle_python, args.python, args.typescript_tools]):
        parser.error("Supply pinned oracle Python, Python runtime and TypeScript tool cache")
    hashes = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in sorted([*CORPUS.glob("*"), *PEERS.glob("*")]) if path.is_file()}
    addon = ROOT / "packages/npm/sdk/native/poolster_node.node"
    if addon.is_file():
        hashes[str(addon.relative_to(ROOT))] = hashlib.sha256(addon.read_bytes()).hexdigest()
    evidence = {"input_hashes": hashes, "corpus": str(CORPUS.relative_to(ROOT)), "cases": [],
                "method": "Owned SDK clients and servers, independent grpcio-tools peer in both directions"}
    try:
        oracle_versions = json.loads(run([args.oracle_python, "-c",
            "import importlib.metadata as m,json;print(json.dumps({n:m.version(n) for n in ['grpcio-tools','grpcio','protobuf']}))"], ROOT))
        expected = {"grpcio-tools":"1.78.0", "grpcio":"1.78.0", "protobuf":"6.33.5"}
        if oracle_versions != expected:
            raise RuntimeError(f"Independent peer runtime differs from pin: {oracle_versions}")
        tools = Path(args.typescript_tools) / "node_modules"
        ts_versions = {name: json.loads((tools / name / "package.json").read_text())["version"]
                       for name in ["@bufbuild/protobuf", "@grpc/grpc-js", "typescript"]}
        if ts_versions != {"@bufbuild/protobuf":"2.16.0", "@grpc/grpc-js":"1.14.6", "typescript":"5.9.3"}:
            raise RuntimeError(f"TypeScript runtime differs from pin: {ts_versions}")
        evidence["runtime_versions"] = {"oracle":oracle_versions, "typescript":ts_versions}
        with tempfile.TemporaryDirectory(prefix="poolster-protobuf-conformance-") as temporary:
            directory = Path(temporary)
            generated = directory / "generated"
            run(["node", str(PEERS / "generate.cjs"), str(generated)], ROOT)
            oracle = directory / "oracle"
            oracle.mkdir()
            include = run([args.oracle_python, "-c",
                           "import grpc_tools,pathlib;print(pathlib.Path(grpc_tools.__file__).parent/'_proto')"], directory).strip()
            run([args.oracle_python, "-m", "grpc_tools.protoc",
                 f"-I{CORPUS}", f"-I{include}", f"--python_out={oracle}",
                 f"--grpc_python_out={oracle}", str(CORPUS / "portable.proto")], directory)
            ts = generated / "typescript"
            (ts / "node_modules").symlink_to(Path(args.typescript_tools).resolve() / "node_modules", target_is_directory=True)
            run([str(ts / "node_modules/.bin/tsc"), "-p", "tsconfig.json"], ts)
            shutil.copyfile(PEERS / "typescript_peer.mjs", ts / "peer.mjs")
            go = generated / "go"
            (go / "cmd/conformance").mkdir(parents=True)
            shutil.copyfile(PEERS / "go_peer.go", go / "cmd/conformance/main.go")
            executable = directory / "go-peer"
            run(["go", "build", "-mod=readonly", "-o", str(executable), "./cmd/conformance"], go)
            base = os.environ.copy()
            oracle_env = {**base, "PYTHONPATH": str(oracle)}
            python_env = {**base, "PYTHONPATH": str(generated / "python/src")}
            oracle_command = [args.oracle_python, str(PEERS / "python_peer.py")]
            commands = {
                "typescript": (["node", str(ts / "peer.mjs")], base),
                "python": ([args.python, str(PEERS / "python_peer.py")], python_env),
                "go": ([str(executable)], base),
            }
            for language, (command, env) in commands.items():
                for server, client, env_server, env_client, label in [
                    (oracle_command, command, oracle_env, env, f"{language}-client-oracle-server"),
                    (command, oracle_command, env, oracle_env, f"oracle-client-{language}-server"),
                ]:
                    evidence["cases"].append(execute_pair(server, client, directory, env_server, env_client, label))
            evidence["status"] = "passed"
    except Exception as error:
        evidence["status"] = "failed"
        evidence["error"] = str(error)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps(evidence))
    return 0 if evidence["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
