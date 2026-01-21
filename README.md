# Spire

<div align="center">

**高性能 Rust 代理/网关**

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./licence)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![GitHub release](https://img.shields.io/github/v/release/lsk569937453/spire)](https://github.com/lsk569937453/spire/releases)

</div>

## 简介

Spire 是一个使用 Rust 编写的高性能代理和网关系统，专注于提供高效、可靠的反向代理服务。基于 Tokio 异步运行时和 Hyper HTTP 库构建，Spire 在性能测试中表现出色，具有低延迟和高吞吐量的特点。

## 主要特性

- **卓越性能** - 基于 Rust 异步运行时，支持高并发连接处理
- **低资源消耗** - 启动内存仅 4MB，运行时内存占用低
- **多协议支持** - HTTP/1.1、HTTP/2、gRPC、TCP/TLS 代理
- **智能路由** - 路径、权重、轮询、随机、头部等多种路由策略
- **安全认证** - API Key、Basic Auth、IP 白名单/黑名单、CORS
- **流量控制** - 令牌桶限流、熔断器保护
- **健康检查** - 主动/被动健康检查，自动故障转移
- **TLS 支持** - HTTPS/TLS 加密连接，ACME 自动证书获取
- **监控指标** - Prometheus 指标导出
- **动态配置** - 支持配置文件热重载

## 功能详解

### 路由策略

| 策略 | 说明 |
|------|------|
| **权重路由** | 按配置权重分配流量，支持灰度发布 |
| **轮询路由** | 依次轮询分发请求，实现负载均衡 |
| **随机路由** | 随机选择后端服务 |
| **头部路由** | 根据请求头信息路由到不同后端 |
| **主机路由** | 基于 Host 头实现虚拟主机路由 |

### 中间件功能

- **认证授权** - API Key、Basic Auth
- **访问控制** - IP 白名单/黑名单
- **限流保护** - 令牌桶算法，支持基于 IP 的限流
- **熔断器** - 自动熔断故障后端
- **CORS** - 跨域资源共享配置
- **请求头操作** - 添加/删除/修改请求头

### 健康检查

- **HTTP 健康检查** - 通过 HTTP 端点检查服务状态
- **TCP 健康检查** - TCP 连接检查
- **主动检查** - 定时主动探测后端
- **被动检查** - 根据请求失败情况判断
- **故障转移** - 自动摘除故障节点，恢复后自动加入

### 协议支持

| 协议 | 说明 |
|------|------|
| HTTP | HTTP/1.1 代理 |
| HTTPS | TLS 加密的 HTTP 代理 |
| TCP | TCP 层代理 |
| HTTP2 | HTTP/2 和 gRPC 代理 |
| HTTP2TLS | TLS 加密的 HTTP/2 代理 |

### ACME 证书自动获取

支持 Let's Encrypt 和 ZeroSSL 的自动证书申请和续期。

## 性能基准测试

基于 Docker Compose 的性能测试，测试条件：100,000 请求，250 并发连接，4核 CPU，8GB 内存限制。

### 测试结果对比

| 代理 | RPS | 平均延迟 | P99 延迟 |
|------|-----|---------|---------|
| Nginx 1.23.3 | 148,677 | 1.6ms | 4.9ms |
| **Spire** | **114,512** | **2.2ms** | **11.8ms** |
| HAProxy 2.7.3 | 85,406 | 2.9ms | 41.7ms |
| Traefik 2.9.8 | 59,487 | 4.1ms | 12.2ms |
| Envoy 1.22.8 | 48,040 | 5.1ms | 51.5ms |
| Caddy 2.6.4 | 13,168 | 18.3ms | 106.5ms |

### 资源消耗

- **启动内存**: 4MB
- **运行内存**: ~35MB（高负载下）
- **二进制大小**: ~7MB（stripped release 构建）

详细测试数据请参考 [benchmarks.md](./benchmarks.md)

## 安装与部署

### 系统要求

- Rust 1.70+ 或使用预编译二进制文件
- Linux / macOS / Windows

### 使用预编译二进制

从 [Releases](https://github.com/lsk569937453/spire/releases) 下载对应平台的二进制文件：

```bash
# Linux
wget https://github.com/lsk569937453/spire/releases/download/v0.0.24/spire-x86_64-unknown-linux-gnu
chmod +x spire-x86_64-unknown-linux-gnu
./spire-x86_64-unknown-linux-gnu -f config.yaml

# macOS (ARM64)
wget https://github.com/lsk569937453/spire/releases/download/v0.0.24/spire-aarch64-apple-darwin
chmod +x spire-aarch64-apple-darwin
./spire-aarch64-apple-darwin -f config.yaml
```

### 从源码构建

```bash
git clone https://github.com/lsk569937453/spire.git
cd spire/rust-proxy
cargo build --release
./target/release/spire -f config.yaml
```

### Docker 部署

```bash
# 使用预构建镜像
docker pull ghcr.io/lsk569937453/spire:latest

# 运行容器
docker run -d \
  -p 6667:6667 \
  -p 9999:9999 \
  -v $(pwd)/config.yaml:/app/config.yaml \
  ghcr.io/lsk569937453/spire:latest
```

## 快速开始

### 最小配置示例

创建 `config.yaml`：

```yaml
log_level: info
admin_port: 9999

servers:
  - listen: 6667
    protocol: http
    routes:
      - route_id: main
        matchers:
          - path:
              value: /
              match_type: prefix
        forward_to:
          kind: single
          target: http://backend-service:8080
```

### 启动服务

```bash
spire -f config.yaml
```

### 高级配置示例

```yaml
log_level: info
admin_port: 9999

servers:
  - listen: 6667
    protocol: https
    domains:
      - example.com
    routes:
      - route_id: api-route
        matchers:
          - path:
              value: /api
              match_type: prefix
          - host:
              value: example.com
        forward_to:
          kind: weight_based
          routes:
            - { endpoint: http://backend1:8080, weight: 70 }
            - { endpoint: http://backend2:8080, weight: 30 }
        health_check:
          kind: http
          path: /health
          interval: 10
          timeout: 5
        middlewares:
          - kind: authentication
            api_key:
              key: X-API-Key
              value: your-secret-key
          - kind: rate_limit
            token_bucket:
              capacity: 100
              rate_per_unit: 100
              unit: second

acme:
  kind: lets_encrypt
```

## 配置参考

### 顶层配置

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `log_level` | string | - | 日志级别: trace/debug/info/warn/error |
| `admin_port` | int | 9999 | 管理接口端口 |
| `health_check_log_enabled` | bool | false | 是否输出健康检查日志 |
| `upstream_timeout_secs` | int | 5000 | 上游请求超时时间（毫秒） |
| `servers` | array | [] | 服务器配置列表 |
| `acme` | object | lets_encrypt | ACME 证书配置 |

### 服务器配置 (servers)

| 参数 | 类型 | 说明 |
|------|------|------|
| `listen` | int | 监听端口 |
| `protocol` | string | 协议类型: http/https/tcp/http2/http2tls |
| `domains` | array | TLS 域名列表 |
| `routes` | array | 路由配置列表 |

### 路由配置 (routes)

| 参数 | 类型 | 说明 |
|------|------|------|
| `route_id` | string | 路由唯一标识 |
| `matchers` | array | 匹配规则列表 |
| `forward_to` | object | 后端路由配置 |
| `health_check` | object | 健康检查配置 |
| `middlewares` | array | 中间件列表 |
| `path_rewrite` | string | 路径重写规则 |
| `timeout` | object | 超时配置 |

### 路由类型 (forward_to.kind)

- `single` - 单个后端
- `weight_based` - 权重路由
- `poll` - 轮询路由
- `random` - 随机路由
- `header_based` - 头部路由

### 中间件类型 (middlewares.kind)

- `authentication` - 认证授权
- `rate_limit` - 限流
- `cors` - CORS
- `allow_deny_list` - IP 访问控制
- `circuit_breaker` - 熔断器

## 管理接口

Spire 在配置的 `admin_port` 上提供 RESTful 管理接口：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/config` | GET | 获取当前配置 |
| `/metrics` | GET | Prometheus 指标 |
| `/certificates` | GET | 获取证书列表 |
| `/certificates` | POST | 上传证书 |

## 开发

### 运行测试

```bash
cd rust-proxy
cargo test
```

### 运行性能测试

```bash
cd benchmarks
docker-compose up
```

### 代码结构

```
rust-proxy/src/
├── main.rs                    # 程序入口
├── vojo/                      # 数据结构定义
├── proxy/                     # 代理核心逻辑
│   ├── http1/                # HTTP/1.1 代理
│   ├── http2/                # HTTP/2 代理
│   └── tcp/                  # TCP 代理
├── middleware/                # 中间件实现
├── health_check/              # 健康检查
├── control_plane/             # 控制平面 API
├── configuration_service/     # 配置管理
└── monitor/                   # 监控指标
```

## 贡献

欢迎贡献代码、报告问题或提出建议！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 开源协议

本项目采用 Apache 2.0 许可证 - 详见 [LICENSE](./licence) 文件。

## 链接

- [Issues](https://github.com/lsk569937453/spire/issues)
- [Releases](https://github.com/lsk569937453/spire/releases)
- [性能测试报告](./benchmarks.md)