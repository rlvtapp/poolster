const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const toml = require('@iarna/toml');
const { GenericToml } = require('release-please/build/src/updaters/generic-toml');
const { GenericJson } = require('release-please/build/src/updaters/generic-json');
const { Generic } = require('release-please/build/src/updaters/generic');
const { Version } = require('release-please/build/src/version');
const { DefaultVersioningStrategy } = require('release-please/build/src/versioning-strategies/default');

const root = path.join(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const config = JSON.parse(read('.github/release-please-config.json')).packages['.'];
const version = Version.parse('0.5.1');

test('release updates inherited workspace and lock versions without changing dependencies', () => {
  assert.equal(config['release-type'], 'simple');
  assert.equal(read(config['version-file']).trim(), toml.parse(read('Cargo.toml')).workspace.package.version);
  const updated = new Map();
  for (const extra of config['extra-files']) {
    const updater = extra.type === 'toml' ? new GenericToml(extra.jsonpath, version)
      : extra.type === 'json' ? new GenericJson(extra.jsonpath, version)
      : new Generic({ version });
    updated.set(extra.path, updater.updateContent(updated.get(extra.path) ?? read(extra.path)));
  }
  const manifest = toml.parse(updated.get('Cargo.toml'));
  assert.equal(manifest.workspace.package.version, '0.5.1');
  const names = new Set(manifest.workspace.members.map(member => {
    const memberManifest = toml.parse(read(`${member}/Cargo.toml`));
    assert.deepEqual(memberManifest.package.version, { workspace: true });
    return memberManifest.package.name;
  }));
  const before = toml.parse(read('Cargo.lock'));
  const after = toml.parse(updated.get('Cargo.lock'));
  let changed = 0;
  for (let i = 0; i < before.package.length; i++) {
    const expected = { ...before.package[i] };
    if (names.has(expected.name)) {
      expected.version = '0.5.1';
      changed++;
    }
    assert.deepEqual(after.package[i], expected);
  }
  assert.equal(changed, names.size);
  const exampleLock = 'examples/custom-plugin/Cargo.lock';
  const exampleBefore = toml.parse(read(exampleLock));
  const exampleAfter = toml.parse(updated.get(exampleLock));
  for (let i = 0; i < exampleBefore.package.length; i++) {
    const expected = { ...exampleBefore.package[i] };
    if (names.has(expected.name)) {
      assert.equal(expected.version, read('version.txt').trim());
      expected.version = '0.5.1';
    }
    assert.deepEqual(exampleAfter.package[i], expected);
  }
  for (const member of manifest.workspace.members) {
    const beforeManifest = toml.parse(read(`${member}/Cargo.toml`));
    const afterManifest = toml.parse(updated.get(`${member}/Cargo.toml`) ?? read(`${member}/Cargo.toml`));
    for (const field of ['dependencies', 'dev-dependencies', 'build-dependencies']) {
      for (const [name, dep] of Object.entries(beforeManifest[field] ?? {})) {
        if (!dep.path || !dep.version) continue;
        assert.equal(dep.version, read('version.txt').trim());
        assert.deepEqual(afterManifest[field][name], { ...dep, version: '0.5.1' }, `${member}: ${field}.${name}`);
      }
    }
  }
  const launcher = JSON.parse(updated.get('packages/npm/cli/package.json'));
  assert.equal(launcher.version, '0.5.1');
  for (const value of Object.values(launcher.optionalDependencies)) assert.equal(value, '0.5.1');
  const sdk = JSON.parse(updated.get('packages/npm/sdk/package.json'));
  assert.equal(sdk.version, '0.5.1');
  for (const value of Object.values(sdk.optionalDependencies)) assert.equal(value, '0.5.1');
  assert.match(updated.get('packages/python/setup.py'), /version="0\.5\.1"/);
});

test('Python prerelease markers update completely when returning to stable releases', () => {
  const source = read('packages/python/setup.py');
  const prerelease = new Generic({ version: Version.parse('0.5.0-alpha.2') }).updateContent(source);
  assert.match(prerelease, /version="0\.5\.0-alpha\.2"/);
  const stable = new Generic({ version }).updateContent(prerelease);
  assert.match(stable, /version="0\.5\.1"/);
  assert.doesNotMatch(stable, /version="0\.5\.1(?:a|b|rc|-)\d/);
});

test('CLI native tests build the compiler first and publication requires Kafka verification and a full release tag build', () => {
  const ci = read('.github/workflows/ci.yml');
  const build = ci.indexOf('go build -o ../target/debug/poolster-openapi .');
  const probe = ci.indexOf('cargo test -p poolster-cli --test local_references');
  assert.ok(build >= 0 && probe > build);
  const release = read('.github/workflows/release-please.yml');
  assert.doesNotMatch(release, /skip-github-release|POOLSTER_RELEASE_ENABLED/);
  assert.match(release, /on:\n  push:\n    branches: \[main\]/);
  const publish = read('.github/workflows/npm-publish.yml');
  assert.match(publish, /publish:\n    name: Publish to npm\n    needs: \[verify, generated-sdk, native, launchers, kafka\]\n    if: \(startsWith\(github\.ref, 'refs\/tags\/v'\) \|\| startsWith\(inputs\.release_tag, 'v'\)\) && !inputs\.windows_only/);
  assert.match(publish, /publish-python:\n    name: Publish to PyPI\n    needs: \[verify, generated-sdk, native, launchers, kafka\]\n    if: \(startsWith\(github\.ref, 'refs\/tags\/v'\) \|\| startsWith\(inputs\.release_tag, 'v'\)\) && !inputs\.windows_only/);
  assert.match(publish, /publish-crates:\n    name: Publish Rust SDK to crates.io\n    needs: \[verify, generated-sdk, native, launchers, kafka\]\n    if: \(startsWith\(github\.ref, 'refs\/tags\/v'\) \|\| startsWith\(inputs\.release_tag, 'v'\)\) && !inputs\.windows_only/);
  const cratesJob = publish.slice(publish.indexOf("  publish-crates:"), publish.indexOf("  publish-python:"));
  assert.match(cratesJob, /permissions:\n      contents: read\n      id-token: write/);
  assert.match(cratesJob, /uses: rust-lang\/crates-io-auth-action@v1/);
  assert.match(cratesJob, /CARGO_REGISTRY_TOKEN: \$\{\{ steps\.crates-auth\.outputs\.token \}\}/);
  assert.doesNotMatch(cratesJob, /secrets\.CRATES_IO_TOKEN/);
  assert.match(publish, /python scripts\/publish-crates.py --tag "\$RELEASE_TAG"/);
});

test('feature releases advance minor versions before 1.0', () => {
  const strategy = new DefaultVersioningStrategy({
    bumpMinorPreMajor: config['bump-minor-pre-major'],
    bumpPatchForMinorPreMajor: config['bump-patch-for-minor-pre-major'],
  });
  const previous = Version.parse('0.4.0');
  const next = strategy.bump(previous, [{ type: 'feat', breaking: false, notes: [] }]);
  assert.equal(next.toString(), '0.5.0');
});


test('publication harness rebuilds immutable tag sources with all gates and optional fixture peers', () => {
  const publish = read('.github/workflows/npm-publish.yml');
  const kafka = read('.github/workflows/kafka-conformance.yml');
  const pinned = "ref: ${{ inputs.release_tag && format('refs/tags/{0}', inputs.release_tag) || github.ref }}";
  assert.equal(publish.match(/uses: actions\/checkout@v7/g).length, publish.split('\n          ' + pinned).length - 1);
  assert.ok(publish.includes("source_ref: ${{ inputs.release_tag && format('refs/tags/{0}', inputs.release_tag) || github.ref }}"));
  assert.ok(kafka.includes('ref: ${{ inputs.source_ref || github.ref }}'));
  assert.ok(publish.includes('test "$REQUESTED_TAG" = "v$(cat version.txt)"'));
  assert.ok(publish.includes('test -z "$RECOVERY_RUN"'));
  assert.ok(publish.includes('--no-save @tanstack/react-query@5.104.1 react@19.3.0 @types/react@19.3.0'));
  assert.ok(publish.includes('RELEASE_TAG: ${{ inputs.release_tag || github.ref_name }}'));
});
