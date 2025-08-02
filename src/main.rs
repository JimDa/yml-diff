use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use serde_yaml::Value;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "yml-diff",
    version = "0.1.0",
    about = "A tiny YAML config file diff tool",
    long_about = "Compare two YAML config files and display differences in a clear, colored format. Perfect for tracking config changes across versions.",
    after_help = "Author: nobody <1085529137@qq.com>"
)]
struct Args {
    /// 旧版本的 YAML 配置文件路径
    #[arg(short, long)]
    old: PathBuf,

    /// 新版本的 YAML 配置文件路径
    #[arg(short, long)]
    new: PathBuf,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigKey(String);

impl Deref for ConfigKey {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for ConfigKey {
    fn from(s: String) -> Self {
        ConfigKey(s)
    }
}

impl From<&String> for ConfigKey {
    fn from(s: &String) -> Self {
        ConfigKey::from(s.clone())
    }
}

impl From<&str> for ConfigKey {
    fn from(s: &str) -> Self {
        ConfigKey(s.to_string())
    }
}

impl AsRef<str> for ConfigKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// 实现自定义排序逻辑
impl Ord for ConfigKey {
    fn cmp(&self, other: &Self) -> Ordering {
        // 按照字典序比较，但考虑层级结构
        // 使用自定义的比较逻辑来处理前缀关系
        self.hierarchical_cmp(other)
    }
}

impl ConfigKey {
    /// 层级化比较：有公共前缀时，按段数排序（段数少的在前）
    fn hierarchical_cmp(&self, other: &Self) -> Ordering {
        let self_parts: Vec<&str> = self.0.split('.').collect();
        let other_parts: Vec<&str> = other.0.split('.').collect();

        // 找到公共前缀的长度
        let common_prefix_len = self_parts
            .iter()
            .zip(other_parts.iter())
            .take_while(|(a, b)| a == b)
            .count();

        // 如果有公共前缀（至少有一段相同）
        if common_prefix_len > 0 {
            // 如果一个是另一个的前缀，短的排在前面
            if common_prefix_len == self_parts.len() && self_parts.len() < other_parts.len() {
                return Ordering::Less;
            }
            if common_prefix_len == other_parts.len() && other_parts.len() < self_parts.len() {
                return Ordering::Greater;
            }

            // 如果有公共前缀但都不是对方的前缀，先按段数排序
            match self_parts.len().cmp(&other_parts.len()) {
                Ordering::Equal => {
                    // 段数相同时，比较第一个不同的部分
                    if common_prefix_len < self_parts.len().min(other_parts.len()) {
                        self_parts[common_prefix_len].cmp(other_parts[common_prefix_len])
                    } else {
                        // 理论上不会到这里，但为了安全起见
                        self.0.cmp(&other.0)
                    }
                }
                other => other, // 段数少的排在前面
            }
        } else {
            // 没有公共前缀，直接按字典序比较
            self.0.cmp(&other.0)
        }
    }
}

impl PartialOrd for ConfigKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct ConfigDiff<'a> {
    added: BTreeMap<ConfigKey, &'a Value>,
    removed: BTreeMap<ConfigKey, &'a Value>,
    modified: BTreeMap<ConfigKey, (&'a Value, &'a Value)>,
}

fn main() -> Result<()> {
    let input = Args::parse();

    let old_val = read_cfg(input.old)?;
    let new_val = read_cfg(input.new)?;

    // 比较 YAML 内容
    let diff = cmp_yml_vals(&old_val, &new_val);

    // 输出结果
    print_diff(&diff);

    Ok(())
}

fn read_cfg(path: PathBuf) -> Result<Value> {
    File::open(&path)
        .map(BufReader::new)
        .map_err(|e| anyhow!("读取配置文件失败！{e}: {:?}", path))
        .and_then(|reader| {
            serde_yaml::from_reader(reader).map_err(|e| anyhow!("解析旧版配置文件失败！{e}"))
        })
}

fn cmp_yml_vals<'a>(old: &'a Value, new: &'a Value) -> ConfigDiff<'a> {
    let old_key_vals = extract_key_vals(old, String::new());
    let new_key_vals = extract_key_vals(new, String::new());

    let old_keys: HashSet<_> = old_key_vals.keys().collect();
    let new_keys: HashSet<_> = new_key_vals.keys().collect();

    let added_keys: Vec<&str> = new_keys
        .difference(&old_keys)
        .map(|&k| k.as_str())
        .collect();

    let added = added_keys
        .into_iter()
        .filter_map(|k| new_key_vals.get(k).map(|&v| (k.into(), v)))
        .collect();

    let removed_keys: Vec<&str> = old_keys
        .difference(&new_keys)
        .map(|&k| k.as_str())
        .collect();

    let removed = removed_keys
        .into_iter()
        .filter_map(|k| old_key_vals.get(k).map(|&v| (k.into(), v)))
        .collect();

    let modified = old_keys
        .intersection(&new_keys)
        .filter_map(|&k| match (old_key_vals.get(k), new_key_vals.get(k)) {
            (Some(&old), Some(&new)) if old != new => Some((k.into(), (old, new))),
            _ => None,
        })
        .collect();

    ConfigDiff {
        added,
        removed,
        modified,
    }
}

fn extract_key_vals(value: &Value, mut prefix: String) -> HashMap<String, &Value> {
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
                        let nested_keys = extract_key_vals(v, prefix.clone());
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
                key_vals.insert(prefix, value);
            }
        }
    }

    key_vals
}

fn get_val_string(val: &Value) -> Cow<str> {
    match val {
        Value::Null => Cow::Borrowed("null"),
        Value::Bool(b) => {
            if *b {
                Cow::Borrowed("true")
            } else {
                Cow::Borrowed("false")
            }
        }
        Value::Number(n) => Cow::Owned(n.to_string()),
        Value::String(s) => Cow::Owned(s.clone()),
        Value::Sequence(seq) => {
            let mut prefix = String::from("[");
            let arr: Vec<_> = seq.iter().map(|v| get_val_string(v)).collect();
            let arr_str = arr.join(", ");
            prefix.push_str(&arr_str);
            prefix.push(']');
            Cow::Owned(prefix)
        }
        Value::Mapping(m) => {
            let map: HashMap<Cow<str>, Cow<str>> = m
                .iter()
                .map(|(k, v)| (get_val_string(k), get_val_string(v)))
                .collect();
            Cow::Owned(format!("{map:?}"))
        }
        Value::Tagged(t) => Cow::Owned(format!("{}:{}", t.tag, get_val_string(&t.value))),
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
    use crate::{cmp_yml_vals, print_diff, read_cfg};
    use std::path::PathBuf;

    #[test]
    fn test_compare_yaml() {
        // 获取项目根目录（Cargo.toml 所在的目录）
        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        // 构建配置文件的完整路径
        let old = PathBuf::from(manifest_dir).join("config_v1.yml");
        let new = PathBuf::from(manifest_dir).join("config_v2.yml");

        println!("Looking for files:");
        println!("Old config: {old:?}");
        println!("New config: {new:?}");

        // 检查文件是否存在
        if !old.exists() {
            panic!(
                "config_v1.yml not found at {:?}. Current working directory: {:?}",
                old,
                std::env::current_dir().unwrap()
            );
        }
        if !new.exists() {
            panic!(
                "config_v2.yml not found at {:?}. Current working directory: {:?}",
                new,
                std::env::current_dir().unwrap()
            );
        }

        let old_val = read_cfg(old).unwrap();
        let new_val = read_cfg(new).unwrap();

        // 比较配置
        let diff = cmp_yml_vals(&old_val, &new_val);

        // 输出结果
        print_diff(&diff);
    }
}
