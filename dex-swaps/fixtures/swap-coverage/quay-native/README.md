Run the independent finalized RPC/PB oracle with Node.js and `bs58` installed, or pass a package.json whose dependencies provide `bs58`:

```sh
node oracle.cjs /absolute/path/to/package.json
```

The oracle reads only these retained fixtures and writes `recomputed-proof.json`. It makes no network requests or database writes. The retained `proof.json` also records publication evidence and executable/source pins.
