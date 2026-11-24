pub(in crate::features::addons) fn tree_entries(tree: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    tree.split(|&byte| byte == 0).filter_map(|entry| {
        let tab = entry.iter().position(|&byte| byte == b'\t')?;
        let (header, path) = entry.split_at(tab);
        Some((header, &path[1..]))
    })
}

pub(in crate::features::addons) fn regular_git_file(header: &[u8]) -> bool {
    header.starts_with(b"100644 ") || header.starts_with(b"100755 ")
}

pub(in crate::features::addons) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
