use std::process::Command;
use std::str;
use regex::Regex;

pub fn find_system_disk() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("lsblk")
        .arg("-o")
        .arg("NAME,TYPE,MOUNTPOINT")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let output_str = str::from_utf8(&output.stdout).unwrap();
        let re = Regex::new(r"(?m)^(\S+)\s+disk").unwrap();
        let mut disks_with_partitions = Vec::new();
        for cap in re.captures_iter(output_str) {
            if output_str.contains(&format!("{} ", &cap[1])) {
                let is_system = output_str.contains(&format!("{} ", &cap[1])) && output_str.contains("/\n");
                disks_with_partitions.push((cap[1].to_string(), is_system));
            }
        }
        return Ok(disks_with_partitions[0].0.clone());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn find_swap_disk() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("lsblk")
        .arg("-f")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let output_str = str::from_utf8(&output.stdout).unwrap();
        let re = Regex::new(r"(?m)^(\S+).*swap").unwrap();
        for cap in re.captures_iter(output_str) {
            return Ok(cap[1].to_string());
        }
        return Err("No swap disk found".into());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn list_disk() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let output = Command::new("lsblk")
        .arg("-dpno")
        .arg("NAME")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let output_str = str::from_utf8(&output.stdout).unwrap();
        let disks: Vec<String> = output_str.lines().map(|s| s.to_string()).collect();
        return Ok(disks);
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}


// 列出所有盘是包含sd,hd,vd,nvme等的，但不包含系统盘和swap盘
pub fn list_data_disk() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let disks = list_disk()?;
    let mut data_disks = Vec::new();

    let system_disk = find_system_disk()?;
    let swap_disk = find_swap_disk()?;

    for disk in disks {
        // 排除系统盘和swap盘
        if disk.contains(&system_disk) {
            continue;
        }

        if disk.contains(&swap_disk) {
            continue;
        }

        if disk.contains("sd") || disk.contains("hd") || disk.contains("vd") || disk.contains("nvme") {
            data_disks.push(disk);
        }
    }
    Ok(data_disks)
}

pub fn install_zfs() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("apt")
        .arg("install")
        .arg("-y")
        .arg("zfsutils-linux")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn install_zfs_pool() -> Result<(), Box<dyn std::error::Error>> {
    let data_disks = list_data_disk()?;
    let mut cmd = Command::new("zpool");
    cmd.arg("create");
    cmd.arg("disk");
    for disk in data_disks {
        cmd.arg(disk);
    }

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn install_zfs_create() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("zfs")
        .arg("create")
        .arg("-o")
        .arg("mountpoint=/root/data")
        .arg("disk/data")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn zfs() -> Result<(), Box<dyn std::error::Error>> {
    install_zfs()?;
    install_zfs_pool()?;
    install_zfs_create()
}

// 写一个find_system_disk的测试用例
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_system_disk() {
        let disk = find_system_disk().unwrap();
        assert_eq!(disk, "sda");
    }
}