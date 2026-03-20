# harvester-dns-controller

A lightweight Kubernetes controller that watches Harvester
`VirtualMachineNetworkConfig` and `LoadBalancer` resources and automatically
registers/deregisters DNS A records in a MikroTik RouterOS or GL.Inet router.

## How it works

Harvester DHCP controller

1. allocates IP to VM NIC
2. writes VirtualMachineNetworkConfig.status.networkConfig[].allocatedIPAddress

harvester-dns-controller (this project)

1. startup: garbage collects any stale DNS records from VMs deleted while down
2. watches VirtualMachineNetworkConfig across all namespaces
3. on VMNC created:   adds our finalizer, waits for IP allocation, creates A record
4. on VMNC updated:   ensures A record is correct (idempotent)
5. on VMNC deleted:   removes DNS record, removes finalizer, allows deletion
6. periodic requeue:  every 10 min drift correction

### DNS record naming

For a VM named `node1` with `DNS_DOMAIN=lab.example.com`:

```
node1.lab.example.com  →  <allocated IP>   TTL 15m
```

VM names are lowercased. The name comes from `spec.vmName` in the VMNC resource,
which matches the Harvester VirtualMachine name and therefore the RKE2/k3s node name.

### Ownership tracking

Every record created by this operator gets the `DNS_COMMENT_TAG` as its RouterOS
comment (default `managed-by=harvester-dns-controller`). The operator only touches
records bearing this comment, leaving manually created entries untouched.

### Finalizer

The operator attaches `dns.geeko.me/cleanup` to every VMNC it manages.
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

| Variable                     | Default                              | Description                                    |
|------------------------------|--------------------------------------|------------------------------------------------|
| `DNS_BACKEND`                | `routeros`                           | DNS backend: `routeros` or `glinet`            |
| `DNS_HOST`                   | `192.168.1.1`                        | Router IP or hostname                          |
| `DNS_USERNAME`               | `admin`                              | API username (RouterOS only)                   |
| `DNS_PASSWORD`               | *(required)*                         | API/admin password                             |
| `DNS_USE_TLS`                | `true`                               | Use HTTPS                                      |
| `DNS_TLS_VERIFY`             | `true`                               | Verify TLS cert                                |
| `DNS_DOMAIN`                 | *(required)*                         | Domain suffix, e.g. `lab.example.com`          |
| `DNS_TTL`                    | `15m`                                | TTL for records (RouterOS format)              |
| `DNS_COMMENT_TAG`            | `managed-by=harvester-dns-controller`| Comment tag on every managed record            |
| `DNS_USE_GUEST_CLUSTER_LABEL`| `true`                               | Use Rancher cluster name as hostname for guest cluster VMs |
| `HEALTH_PORT`                | `8080`                               | Port for `/healthz` and `/readyz`              |
| `WATCH_NAMESPACES`           | *(empty = all)*                      | Comma-separated namespace list                 |
| `LOG_FORMAT`                 | *(empty = human)*                    | Set `json` for structured logging              |
| `RUST_LOG`                   | *(default)*                          | Log level filter                               |

## Running locally

### Using RouterOS backend

```bash
export KUBECONFIG=~/.kube/harvester.yaml
export DNS_BACKEND=routeros
export DNS_HOST=192.168.1.1
export DNS_USERNAME=dns-operator
export DNS_PASSWORD=secret
export DNS_USE_TLS=false
export DNS_TLS_VERIFY=false
export DNS_DOMAIN=lab.example.com
export RUST_LOG=harvester_dns_controller=debug

cargo run
```

### Using GL.Inet backend

```bash
export KUBECONFIG=~/.kube/harvester.yaml
export DNS_BACKEND=glinet
export DNS_HOST=192.168.8.1
export DNS_PASSWORD=secret
export DNS_USE_TLS=false
export DNS_TLS_VERIFY=false
export DNS_DOMAIN=lab.example.com
export RUST_LOG=harvester_dns_controller=debug

cargo run
```

## Building (requires Rust 1.85+)

```bash
cargo build --release
docker build -t your-registry/harvester-dns-controller:0.1.0 .
docker push your-registry/harvester-dns-controller:0.1.0
```

## Deploying with Helm

```bash
helm install harvester-dns-controller ./chart/harvester-dns-controller \
  --namespace kube-system \
  --set image.repository=your-registry/harvester-dns-controller \
  --set image.tag=0.1.0 \
  --set routeros.host=192.168.1.1 \
  --set routeros.username=dns-operator \
  --set routeros.password=secret \
  --set routeros.useTls=false \
  --set routeros.tlsVerify=false \
  --set dns.domain=lab.example.com
```
