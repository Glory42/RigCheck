//! PCI IDs for the hardware table's GPUs, from pci.ids (the PCI ID
//! Repository), so detected GPUs can be identified exactly.

use rigcheck_core::Catalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PciId {
    pub vendor_id: u16,
    pub device_id: u16,
    /// The hardware table entry it belongs to.
    pub name: String,
}

/// Every pci.ids device whose bracketed name matches exactly one table GPU.
/// IDs shared by several table entries ("Radeon RX 470/480/570/580") are
/// skipped rather than guessed.
pub fn ids(catalog: &Catalog, pci_ids: &str) -> Vec<PciId> {
    let mut found = Vec::new();
    let mut vendor_id = None;
    for line in pci_ids.lines() {
        if line.starts_with('#') || line.trim().is_empty() || line.starts_with("\t\t") {
            continue; // comments and subsystem lines
        }
        let Some(device) = line.strip_prefix('\t') else {
            vendor_id = u16::from_str_radix(&line[..4.min(line.len())], 16).ok();
            continue;
        };
        let (Some(vendor_id), Some((id, name))) = (vendor_id, device.split_once("  ")) else {
            continue;
        };
        let Some(bracketed) = name
            .split_once('[')
            .and_then(|(_, rest)| rest.strip_suffix(']'))
        else {
            continue;
        };
        if let ([entry], Ok(device_id)) = (
            catalog.match_gpus(bracketed).as_slice(),
            u16::from_str_radix(id, 16),
        ) {
            found.push(PciId {
                vendor_id,
                device_id,
                name: entry.name.clone(),
            });
        }
    }
    found
}
