# routeros-dns-operator

A lightweight Kubernetes controller that watches Harvester
`VirtualMachineNetworkConfig` resources and automatically registers/deregisters
DNS A records in a MikroTik RouterOS DNS server via the RouterOS REST API.

## How it works

```
Harvester DHCP controller
  → allocates IP to VM NIC
  → writes VirtualMachineNetworkConfig.status.networkConfig[].allocatedIPAddress

routeros-dns-operator (this project)
  → startup: garbage collects any stale DNS records from VMs deleted while down
  → watches VirtualMachineNetworkConfig across all namespaces
  → on VMNC created:   adds our finalizer, waits for IP allocation, creates A record
  → on VMNC updated:   ensures A record is correct (idempotent)
  → on VMNC deleted:   removes DNS record, removes finalizer, allows deletion
  → periodic requeue:  every 10 min drift correction
  → health endpoint:   GET /healthz and /readyz on :8080
```

### DNS record naming

For a VM named `node1` with `DNS_DOMAIN=lab.example.com`:

```
node1.lab.example.com  →  <allocated IP>   TTL 15m
```

VM names are lowercased. The name comes from `spec.vmName` in the VMNC resource,
which matches the Harvester VirtualMachine name and therefore the RKE2/k3s node name.

### Ownership tracking

Every record created by this operator gets the `DNS_COMMENT_TAG` as its RouterOS
comment (default `managed-by=routeros-dns-operator`). The operator only touches
records bearing this comment, leaving manually created entries untouched.

### Finalizer

The operator attaches `dns.routeros.geeko.me/cleanup` to every VMNC it manages.
When Kubernetes processes a deletion, it holds the object open until the operator
has removed the DNS record and released the finalizer — no ghost records.

## RouterOS prerequisites

RouterOS v7.1+ is required for the REST API:

```routeros
# Enable HTTP (use www-ssl in production)
/ip service enable www

# Dedicated API user
/user group add name=dns-api policy=read,write,api,!local,!telnet,!ssh,!ftp,!reboot,!password,!sensitive,!sniff,!test
/user add name=dns-operator group=dns-api password=<your-secret>
```

## Configuration

| Variable              | Default                              | Description                                    |
|-----------------------|--------------------------------------|------------------------------------------------|
| `ROUTEROS_HOST`       | `192.168.1.1`                        | RouterOS IP or hostname                        |
| `ROUTEROS_USERNAME`   | `admin`                              | RouterOS API username                          |
| `ROUTEROS_PASSWORD`   | *(required)*                         | RouterOS API password                          |
| `ROUTEROS_USE_TLS`    | `true`                               | Use HTTPS                                      |
| `ROUTEROS_TLS_VERIFY` | `true`                               | Verify TLS cert                                |
| `DNS_DOMAIN`          | *(required)*                         | Domain suffix, e.g. `lab.example.com`          |
| `DNS_TTL`             | `15m`                                | TTL for records (RouterOS format)              |
| `DNS_COMMENT_TAG`     | `managed-by=routeros-dns-operator`   | Comment tag on every managed record            |
| `HEALTH_PORT`         | `8080`                               | Port for `/healthz` and `/readyz`              |
| `WATCH_NAMESPACES`    | *(empty = all)*                      | Comma-separated namespace list                 |
| `LOG_FORMAT`          | *(empty = human)*                    | Set `json` for structured logging              |
| `RUST_LOG`            | *(default)*                          | Log level filter                               |

## Running locally

```bash
export KUBECONFIG=~/.kube/harvester.yaml
export ROUTEROS_HOST=192.168.1.1
export ROUTEROS_USERNAME=dns-operator
export ROUTEROS_PASSWORD=secret
export ROUTEROS_USE_TLS=false
export ROUTEROS_TLS_VERIFY=false
export DNS_DOMAIN=lab.example.com
export RUST_LOG=routeros_dns_operator=debug

cargo run
```

## Building (requires Rust 1.85+)

```bash
cargo build --release
docker build -t your-registry/routeros-dns-operator:0.1.0 .
docker push your-registry/routeros-dns-operator:0.1.0
```

## Deploying with Helm

```bash
helm install routeros-dns-operator ./chart/routeros-dns-operator \
  --namespace kube-system \
  --set image.repository=your-registry/routeros-dns-operator \
  --set image.tag=0.1.0 \
  --set routeros.host=192.168.1.1 \
  --set routeros.username=dns-operator \
  --set routeros.password=secret \
  --set routeros.useTls=false \
  --set routeros.tlsVerify=false \
  --set dns.domain=lab.example.com
```

## Project structure

```
src/
  main.rs         — entrypoint: logging, startup GC, health endpoint, controller loop
  config.rs       — configuration from environment
  types.rs        — Harvester VMNC CRD types + RouterOS API types
  controller.rs   — reconcile, finalizer management, startup GC, error policy
  routeros.rs     — RouterOS REST API client (CRUD /ip/dns/static)
  health.rs       — HTTP /healthz and /readyz endpoint
chart/routeros-dns-operator/
  Chart.yaml
  values.yaml
  templates/      — ServiceAccount, ClusterRole, ClusterRoleBinding, Secret, Deployment
Cargo.toml
Dockerfile
deploy.yaml
README.md
```
