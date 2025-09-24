# Spire - 高性能 Rust 代理/网关

## 项目简介

Spire 是一个使用 Rust 编写的高性能代理和网关系统，专注于提供高效、可靠的反向代理服务。基于异步运行时构建，Spire 在性能测试中表现出色，具有低延迟和高吞吐量的特点。

## 主要特性

- **高性能**: 基于 Rust 编写，提供卓越的性能表现
- **低资源消耗**: 内存占用少，启动内存仅需 4MB
- **高并发处理**: 支持大量并发连接
- **灵活配置**: 通过 YAML 配置文件进行灵活配置
- **TLS/SSL 支持**: 支持 HTTPS/TLS 加密连接
- **健康检查**: 内置健康检查机制
- **监控指标**: 提供性能监控和指标收集
- **OpenAPI 集成**: 支持 OpenAPI 规范转换

## Spire 完成的具体功能

### 1. 多协议支持
- HTTP/1.1 和 HTTP/2 代理
- TCP 代理功能
- TLS/HTTPS 支持
- 支持多个监听端口和协议

### 2. 智能路由功能
- **路径匹配**: 基于URL路径的请求匹配和路由
- **权重路由**: 支持基于权重的负载均衡
- **轮询路由**: 轮询方式分发请求
- **随机路由**: 随机方式分发请求
- **头部路由**: 根据请求头信息进行路由
- **主机名路由**: 基于 Host 头进行虚拟主机路由

### 3. 安全与认证
- **API 密钥认证**: 支持基于 API 密钥的访问控制
- **基本认证**: 支持 HTTP Basic Authentication
- **IP 访问控制**: 支持 IP 地址白名单/黑名单机制
- **CORS 支持**: 跨域资源共享控制
- **请求头处理**: 在请求和响应中操作头部信息

### 4. 流量控制
- **限流功能**: 基于令牌桶算法的请求限流
- **熔断机制**: 电路断路器模式保护后端服务
- **请求头转发**: 透明转发原始客户端信息

### 5. 健康检查与监控
- **主动健康检查**: 定期检查后端服务状态
- **被动健康检查**: 根据请求结果判断服务状态
- **故障转移**: 自动将流量导向健康的后端服务
- **性能指标**: 收集并报告性能和可用性指标

### 6. 高级功能
- **路径重写**: 支持在转发请求时重写 URL 路径
- **超时配置**: 可配置上游服务的请求超时时间
- **服务发现**: 支持动态后端服务发现
- **gRPC 代理**: 支持 gRPC 请求的代理和负载均衡
- **OpenAPI 转换**: 提供 OpenAPI 规范到代理配置的转换功能
- **ACME 协议支持**: 支持 Let's Encrypt 等证书自动获取

## 性能基准测试

根据基准测试结果（使用 100,000 请求，250 并发），Spire 表现优异：

- **请求/秒**: 114,511 RPS
- **平均响应时间**: 2.2 毫秒
- **启动内存**: 仅 4MB
- **峰值内存**: 35MB

在与 Nginx、Envoy、HAProxy 等主流代理的对比中，Spire 展现了卓越的性能优势。

## 安装与构建

### 系统要求

- Rust 1.70+ 
- Cargo 包管理器

### 构建项目

```bash
# 克隆项目
git clone https://github.com/lsk569937453/spire.git

# 进入项目目录
cd spire/rust-proxy

# 构建项目
cargo build --release
```

### Docker 部署

```bash
# 使用预构建的 Docker 镜像
docker pull lsk569937453/spire

# 运行容器
docker run -d -p 6667:6667 --name spire lsk569937453/spire
```

## 快速开始

### 1. 配置文件示例

创建一个配置文件 `config.yaml`：

```yaml
log_level: info
admin_port: 9999
servers:
  - listen: 6667
    protocol: http
    routes:
      - match:
          prefix: /
        forward_to: http://backend-service:8080
```

### 2. 启动 Spire

```bash
# 使用配置文件启动
./spire -f config.yaml
```

### 3. 验证运行

服务启动后，Spire 将在配置的端口上监听请求，并根据路由配置进行转发。

## 配置说明

Spire 使用 YAML 格式进行配置，主要配置项包括：

- `log_level`: 日志级别 (trace, debug, info, warn, error)
- `admin_port`: 管理接口端口
- `servers`: 服务器配置列表
  - `listen`: 监听端口
  - `protocol`: 协议类型 (http, https)
  - `routes`: 路由规则
    - `match`: 匹配规则
    - `forward_to`: 后端服务地址

## 开发与测试

### 运行单元测试

```bash
cargo test
```

### 运行性能测试

项目包含基准测试套件，可在 `benchmarks` 目录下找到各种代理的性能对比测试。

## 贡献

欢迎提交 Issue 和 Pull Request 来改进 Spire。请遵循以下步骤：

1. Fork 项目
2. 创建功能分支
3. 提交更改
4. 发起 Pull Request

## 许可证

本项目采用 Apache 2.0 许可证 - 详情请参见 [LICENSE](./licence) 文件。

## 支持

如果您遇到问题或有任何疑问，请：

- 查看 Issues 页面
- 提交新的 Issue
- 联系项目维护者

## 致谢

感谢所有为 Spire 项目做出贡献的开发者，以及 Rust 社区提供的优秀生态系统。