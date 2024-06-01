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

    let system_disk = match find_system_disk() {
        Ok(disk) => disk,
        Err(_) => "no_system_disk".to_string(),
    };
    let swap_disk = match find_swap_disk() {
        Ok(disk) => disk,
        Err(_) => "no_swap_disk".to_string(),
    };

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
    // 判断是否已经安装zfs
    match Command::new("zfs").output() {
        Ok(_) => return Ok(()),
        Err(_) => (),
    };

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

pub fn remove_disk_from_fstab(disk: &str) -> Result<(), Box<dyn std::error::Error>> {
    let sed_arg = format!("/.*{}.*/d", disk.replace("/", r"\/"));
    let output = Command::new("sed")
        .arg("-i")
        .arg(sed_arg)
        .arg("/etc/fstab")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}


pub fn add_disk_to_fstab(disk: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open("/etc/fstab")?;

    let line = format!("/dev/{} /root/data xfs defaults,prjquota 0 0\n", disk);
    file.write_all(line.as_bytes())?;

    Ok(())
}

pub fn disk_size(disk: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let output = Command::new("lsblk")
        .arg("-b")
        .arg("-o")
        .arg("SIZE")
        .arg("--noheadings")
        .arg(disk)
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8(output.stderr).unwrap();
        return Err(error.into());
    }

    let output_str = str::from_utf8(&output.stdout).unwrap();
    let size_str = output_str.lines().next().unwrap_or("").trim();
    let size = size_str.parse::<u64>()?;

    Ok(size)
}

pub fn install_zfs_pool() -> Result<(), Box<dyn std::error::Error>> {
    let data_disks = list_data_disk()?;
    let mut cmd = Command::new("zpool");
    cmd.arg("create");
    cmd.arg("disk");
    cmd.arg("-f");
    for disk in data_disks {
        remove_disk_from_fstab(&disk)?;
        cmd.arg(disk);
    }

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn get_zfs_free_space(pool: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let output = Command::new("zpool")
        .arg("list")
        .arg("-H")
        .arg("-o")
        .arg("free")
        .arg("-p")
        .arg(pool)
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8(output.stderr).unwrap();
        return Err(error.into());
    }

    let output_str = str::from_utf8(&output.stdout).unwrap();
    let free_space_str = output_str.trim();
    let free_space = free_space_str.parse::<u64>()?;

    Ok(free_space)
}

pub fn install_zfs_create() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("zfs")
        .arg("create")
        .arg("-V")
        .arg(format!("{}",get_zfs_free_space("disk")? * 93 / 100))
        .arg("disk/data")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn install_xfs() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("mkfs.xfs")
        .arg("-f")
        .arg("/dev/zd0")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();

    println!("{}", error);
    Err(error.into())
}

pub fn install_mkdir_dir() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("mkdir")
        .arg("-p")
        .arg("/root/data")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn install_mount_dir() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("mount")
        .arg("-t")
        .arg("xfs")
        .arg("-o")
        .arg("defaults,prjquota")
        .arg("/dev/zd0")
        .arg("/root/data")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        return Ok(());
    }

    let error = String::from_utf8(output.stderr).unwrap();
    Err(error.into())
}

pub fn set_docker_disk() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("systemctl")
        .arg("stop")
        .arg("docker")
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        let error = String::from_utf8(output.stderr).unwrap();
        return Err(error.into());
    }

    let output = Command::new("mv")
        .arg("/var/lib/docker")
        .arg("/root/data/docker")
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        let error = String::from_utf8(output.stderr).unwrap();
        return Err(error.into());
    }

    let output = Command::new("ln")
        .arg("-s")
        .arg("/root/data/docker")
        .arg("/var/lib/docker")
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        let error = String::from_utf8(output.stderr).unwrap();
        return Err(error.into());
    }

    let output = Command::new("systemctl")
        .arg("start")
        .arg("docker")
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
    install_zfs_create()?;
    install_xfs()?;
    install_mkdir_dir()?;
    install_mount_dir()?;
    remove_disk_from_fstab("zd0")?;
    add_disk_to_fstab("zd0")?;
    set_docker_disk()?;

    Ok(())
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