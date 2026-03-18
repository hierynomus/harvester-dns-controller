//! Shared helper functions for controllers.

use std::collections::BTreeMap;

use kube::{Api, Client};
use tracing::warn;

use crate::kubernetes::{VirtualMachine, VmNetworkConfig};

/// Look up the parent VirtualMachine's labels from a VmNetworkConfig's ownerReferences.
pub async fn get_vm_labels(
    kube: &Client,
    vmnetcfg: &VmNetworkConfig,
) -> Option<BTreeMap<String, String>> {
    let namespace = vmnetcfg.metadata.namespace.as_ref()?;
    let owner_refs = vmnetcfg.metadata.owner_references.as_ref()?;

    // Find the VirtualMachine owner reference
    let vm_ref = owner_refs
        .iter()
        .find(|r| r.kind == "VirtualMachine" && r.api_version.starts_with("kubevirt.io/"))?;

    let vm_api: Api<VirtualMachine> = Api::namespaced(kube.clone(), namespace);

    match vm_api.get(&vm_ref.name).await {
        Ok(vm) => vm.metadata.labels,
        Err(e) => {
            warn!(vm = %vm_ref.name, error = %e, "Failed to look up parent VM");
            None
        }
    }
}
