use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use serde_yaml::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::thread;

#[derive(Parser)]
#[command(
    name = "yml-diff",
    author = "nobody <1085529137@qq.com>",
    version,
    about = "A tiny YAML configuration file diff tool",
    long_about = "Compare two YAML configuration files and display differences in a clear, colored format. Perfect for tracking configuration changes across versions."
)]
struct Args {
    /// 旧版本的 YAML 配置文件路径
    #[arg(short, long)]
    old: PathBuf,

    /// 新版本的 YAML 配置文件路径
    #[arg(short, long)]
    new: PathBuf,
}

struct ConfigDiff<'a> {
    added: BTreeMap<String, &'a Value>,
    removed: BTreeMap<String, &'a Value>,
    modified: BTreeMap<String, (&'a Value, &'a Value)>,
}

fn main() -> Result<()> {
    let input = Args::parse();

    let (old_content, new_content) = read_config_content(&input.old, &input.new)?;

    // 解析 YAML 内容
    let old_yaml: Value = serde_yaml::from_str(&old_content).context("解析旧配置文件失败")?;

    let new_yaml: Value = serde_yaml::from_str(&new_content).context("解析新配置文件失败")?;

    // 比较配置
    let diff = compare_yaml_vals(&old_yaml, &new_yaml);

    // 输出结果
    print_diff(&diff);

    Ok(())
}

fn read_config_content(old: &PathBuf, new: &PathBuf) -> Result<(String, String)> {
    thread::scope(|s| {
        let old_handle = s.spawn(|| -> Result<String> {
            fs::read_to_string(old).with_context(|| format!("无法读取旧配置文件: {:?}", old))
        });

        let new_handle = s.spawn(|| -> Result<String> {
            fs::read_to_string(new).with_context(|| format!("无法读取新配置文件: {:?}", new))
        });

        // 等待两个线程完成并获取结果
        let old_content = old_handle
            .join()
            .map_err(|_| anyhow::anyhow!("读取旧配置文件的线程发生错误"))??;
        let new_content = new_handle
            .join()
            .map_err(|_| anyhow::anyhow!("读取新配置文件的线程发生错误"))??;

        Ok::<(String, String), anyhow::Error>((old_content, new_content))
    })
}

fn compare_yaml_vals<'a>(old: &'a Value, new: &'a Value) -> ConfigDiff<'a> {
    let old_key_vals = extract_all_keys(&old, String::new());
    let new_key_vals = extract_all_keys(&new, String::new());

    let old_keys: HashSet<_> = old_key_vals.keys().map(|s| s.as_str()).collect();
    let new_keys: HashSet<_> = new_key_vals.keys().map(|s| s.as_str()).collect();

    let added: Vec<&str> = new_keys.difference(&old_keys).copied().collect();

    let added_key_vals = added
        .into_iter()
        .filter_map(|k| new_key_vals.get(k).map(|&v| (k.into(), v)))
        .collect();

    let removed: Vec<&str> = old_keys.difference(&new_keys).copied().collect();

    let removed_key_vals = removed
        .into_iter()
        .filter_map(|k| old_key_vals.get(k).map(|&v| (k.into(), v)))
        .collect();

    let modified_key_vals = old_keys
        .intersection(&new_keys)
        .into_iter()
        .filter_map(|&k| {
            old_key_vals
                .get(k)
                .map(|&v| (k, v))
                .and_then(|(k, old)| new_key_vals.get(k).map(|&new| (k.into(), (old, new))))
                .filter(|(_, (old, new))| old != new)
        })
        .collect();

    ConfigDiff {
        added: added_key_vals,
        removed: removed_key_vals,
        modified: modified_key_vals,
    }
}

fn extract_all_keys(value: &Value, mut prefix: String) -> HashMap<String, &Value> {
    let mut key_vals = HashMap::new();

    match value {
        Value::Mapping(map) => {
            let prefix_len = prefix.len();
            for (k, v) in map {
                if let Some(key_str) = k.as_str() {
                    // 重用 prefix String，避免重复分配
                    if !prefix.is_empty() {
                        prefix.push('.');
                    }
                    prefix.push_str(key_str);

                    // 递归处理嵌套对象
                    if let Value::Mapping(_) = v {
                        let nested_keys = extract_all_keys(v, prefix.clone());
                        key_vals.extend(nested_keys);
                    } else {
                        // 添加当前键值对
                        key_vals.insert(prefix.clone(), v);
                    }

                    // 恢复 prefix 到之前的状态，重用 String
                    prefix.truncate(prefix_len);
                }
            }
        }
        _ => {
            // 如果不是映射类型，直接添加
            if !prefix.is_empty() {
                key_vals.insert(prefix, &value);
            }
        }
    }

    key_vals
}

fn get_val_string(val: &Value) -> String {
    match val {
        Value::Null => String::from("null"),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Sequence(seq) => {
            let mut prefix = String::from("[");
            let arr: Vec<_> = seq.iter().map(|v| get_val_string(&v)).collect();
            let arr_str = arr.join(", ");
            prefix.push_str(&arr_str);
            prefix.push_str("]");
            prefix
        }
        Value::Mapping(m) => {
            let map: HashMap<String, String> = m
                .iter()
                .map(|(k, v)| (get_val_string(k), get_val_string(v)))
                .collect();
            format!("{:?}", map)
        }
        Value::Tagged(t) => {
            format!("{}:{}", t.tag, get_val_string(&t.value))
        }
    }
}

fn print_diff(diff: &ConfigDiff) {
    println!("{}", "=== YAML 配置文件差异报告 ===".bold());
    println!();

    // 统计信息
    println!("{}", "统计信息:".blue().bold());
    println!("  新增: {}", diff.added.len().to_string().green());
    println!("  删除: {}", diff.removed.len().to_string().red());
    println!("  修改: {}", diff.modified.len().to_string().yellow());

    println!();

    if !diff.added.is_empty() {
        println!("{}", "新增的配置项:".green().bold());
        for (key, &val) in &diff.added {
            println!("  + {}: {}", key.green(), get_val_string(val).green());
        }
        println!();
    }

    if !diff.removed.is_empty() {
        println!("{}", "删除的配置项:".red().bold());
        for (key, &val) in &diff.removed {
            println!("  - {}: {}", key.red(), get_val_string(val).red());
        }
        println!();
    }

    if !diff.modified.is_empty() {
        println!("{}", "修改的配置项:".yellow().bold());
        for (key, (old, new)) in &diff.modified {
            println!("  ~ {}", key.yellow());
            println!("  修改前 {}", get_val_string(old).yellow());
            println!("  修改后 {}", get_val_string(new).yellow());
        }
        println!();
    }

    if diff.added.is_empty() && diff.removed.is_empty() && diff.modified.is_empty() {
        println!("{}", "没有发现配置差异".green());
    }
}

#[cfg(test)]
mod tests {
    use crate::{compare_yaml_vals, print_diff, read_config_content};
    use serde_yaml::Value;
    use std::path::PathBuf;
    use std::str::FromStr;

    #[test]
    fn test_compare_yaml() {
        let old =
            PathBuf::from_str("/Users/dapengchengsmac/RustroverProjects/yml-diff/config_v1.yml")
                .unwrap();
        let new =
            PathBuf::from_str("/Users/dapengchengsmac/RustroverProjects/yml-diff/config_v2.yml")
                .unwrap();

        let (old_content, new_content) = read_config_content(&old, &new).expect("读取失败");

        // 解析 YAML 内容
        let old_yaml: Value = serde_yaml::from_str(&old_content).expect("解析旧配置文件失败");

        let new_yaml: Value = serde_yaml::from_str(&new_content).expect("解析新配置文件失败");

        // 比较配置
        let diff = compare_yaml_vals(&old_yaml, &new_yaml);

        // 输出结果
        print_diff(&diff);
    }
}
