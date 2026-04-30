export const architectureMermaid = `graph TB
    subgraph Client["用户客户端"]
        A[Claude Desktop / Cursor / Continue]
        B[API Key: sk-local-xxxxx]
        C[Base URL: http://localhost:8787]
    end

    subgraph ProxyServer["本地 API 代理服务<br/>(Rust HTTP Server)"]
        D[请求验证]
        E[请求解析]
        F[供应商选择]
        G[请求转发]
        H[响应处理]
        I[自动切换]
        J[健康检查器]
    end

    subgraph Providers["供应商"]
        K[Claude Provider]
        L[OpenAI Provider]
        M[Gemini Provider]
    end

    Client -->|HTTP Request| D
    D --> E --> F --> G --> H
    H -->|失败| I
    I -->|重试| F
    H -->|成功| Client

    J -.定时检查.-> K
    J -.定时检查.-> L
    J -.定时检查.-> M

    G -->|转发| K
    G -->|转发| L
    G -->|转发| M
    K -->|响应| H
    L -->|响应| H
    M -->|响应| H
`;
