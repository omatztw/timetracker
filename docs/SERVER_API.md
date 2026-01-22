# TimeTracker Server API Specification

このドキュメントは、TimeTrackerデスクトップアプリからのデータを受け取るサーバー側APIの仕様です。

## アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│  ┌──────────────┐                  ┌──────────────────┐        │
│  │  TimeTracker │  POST /api/upload│  API Server      │        │
│  │  (Desktop)   │ ────────────────→│                  │        │
│  └──────────────┘                  └────────┬─────────┘        │
│         │                                   │                  │
│  Windows ログインユーザー名を               │ DB保存           │
│  user_id として送信                         ▼                  │
│                                    ┌──────────────────┐        │
│  ※ 10分未満の短時間データは        │  Database        │        │
│    クライアント側でフィルタリング   │  (user_id で紐付け)│        │
│                                    └──────────────────┘        │
│  ┌──────────────┐     SAML                 │                  │
│  │  ブラウザ     │ ←────────────→          │                  │
│  │              │                          ▼                  │
│  └──────────────┘                  ┌──────────────────┐        │
│         │                          │  Web UI          │        │
│         └─────────────────────────→│  (SAML認証済み)   │        │
│              閲覧（自分のデータ）    └──────────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

## データフィルタリング

デスクトップアプリは以下のルールでデータを集計・フィルタリングしてからアップロードします：

| カテゴリ | フィルタリングルール |
|---------|---------------------|
| **ブラウザ以外のアプリ** | アプリ別に合計使用時間を集計し、閾値（デフォルト10分）以上のもののみ |
| **ブラウザ** | ドメイン別に合計閲覧時間を集計し、閾値（デフォルト10分）以上のもののみ |

**ブラウザとして認識されるプロセス:**
- chrome.exe, msedge.exe, firefox.exe, brave.exe, opera.exe, vivaldi.exe, iexplore.exe

---

## API Endpoints

### POST /api/upload

デスクトップアプリから集計済みアクティビティデータを受け取ります。

#### Request

**Headers:**
```
Content-Type: application/json
```

**Body:**
```json
{
  "user_id": "user@domain.com",
  "machine_name": "PC-WORKSTATION01",
  "date": "2024-01-15",
  "min_duration_seconds": 600,
  "app_summaries": [
    {
      "process_name": "Code.exe",
      "total_seconds": 7200
    },
    {
      "process_name": "slack.exe",
      "total_seconds": 3600
    }
  ],
  "domain_summaries": [
    {
      "domain": "github.com",
      "total_seconds": 1800
    },
    {
      "domain": "stackoverflow.com",
      "total_seconds": 900
    }
  ]
}
```

#### Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `user_id` | string | Yes | Windowsログインユーザー名（UPN形式: `user@domain.com` または SAM形式: `DOMAIN\user`） |
| `machine_name` | string | No | マシン名（`COMPUTERNAME` 環境変数） |
| `date` | string | Yes | 対象日（`YYYY-MM-DD`形式） |
| `min_duration_seconds` | integer | Yes | フィルタリングに使用した閾値（秒） |
| `app_summaries` | array | Yes | アプリ使用時間サマリー（ブラウザ以外） |
| `domain_summaries` | array | Yes | ドメイン閲覧時間サマリー（ブラウザ） |

#### App Summary Fields

| Field | Type | Description |
|-------|------|-------------|
| `process_name` | string | プロセス名（例: `Code.exe`, `slack.exe`） |
| `total_seconds` | integer | その日の合計使用時間（秒） |

#### Domain Summary Fields

| Field | Type | Description |
|-------|------|-------------|
| `domain` | string | ドメイン名（例: `github.com`） |
| `total_seconds` | integer | その日の合計閲覧時間（秒） |

#### Response

**Success (200 OK):**
```json
{
  "success": true,
  "message": "Received 3 apps, 2 domains for user@domain.com on 2024-01-15"
}
```

**Error (4xx/5xx):**
```json
{
  "success": false,
  "message": "Error description",
  "error_code": "INVALID_USER"
}
```

#### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `INVALID_USER` | 400 | user_id が不正または空 |
| `INVALID_DATA` | 400 | リクエストデータが不正 |
| `USER_NOT_FOUND` | 404 | 登録されていないユーザー（オプション） |
| `SERVER_ERROR` | 500 | サーバー内部エラー |

---

## データベーススキーマ例

### PostgreSQL

```sql
-- ユーザーテーブル（SAML認証と紐付け）
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(255) UNIQUE NOT NULL,  -- UPN or SAM format
    email VARCHAR(255),
    display_name VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_upload_at TIMESTAMP
);

-- アプリ使用時間テーブル
CREATE TABLE app_usage (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    machine_name VARCHAR(255),
    date DATE NOT NULL,
    process_name VARCHAR(255) NOT NULL,
    total_seconds INTEGER NOT NULL,
    uploaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id),
    CONSTRAINT unique_app_usage UNIQUE (user_id, machine_name, date, process_name)
);

-- ドメイン閲覧時間テーブル
CREATE TABLE domain_usage (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    machine_name VARCHAR(255),
    date DATE NOT NULL,
    domain VARCHAR(255) NOT NULL,
    total_seconds INTEGER NOT NULL,
    uploaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id),
    CONSTRAINT unique_domain_usage UNIQUE (user_id, machine_name, date, domain)
);

-- インデックス
CREATE INDEX idx_app_usage_user_date ON app_usage(user_id, date);
CREATE INDEX idx_domain_usage_user_date ON domain_usage(user_id, date);
```

---

## サーバー実装例

### Python (FastAPI)

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import Optional, List
from datetime import date

app = FastAPI()

class AppSummary(BaseModel):
    process_name: str
    total_seconds: int

class DomainSummary(BaseModel):
    domain: str
    total_seconds: int

class UploadRequest(BaseModel):
    user_id: str
    machine_name: Optional[str]
    date: str
    min_duration_seconds: int
    app_summaries: List[AppSummary]
    domain_summaries: List[DomainSummary]

class UploadResponse(BaseModel):
    success: bool
    message: str

@app.post("/api/upload", response_model=UploadResponse)
async def upload_activities(request: UploadRequest):
    if not request.user_id:
        raise HTTPException(status_code=400, detail="Invalid user_id")

    app_count = len(request.app_summaries)
    domain_count = len(request.domain_summaries)

    if app_count == 0 and domain_count == 0:
        return UploadResponse(
            success=True,
            message="No data to store"
        )

    # TODO: データベースに保存（UPSERT）
    # await upsert_app_usage(request.user_id, request.machine_name, request.date, request.app_summaries)
    # await upsert_domain_usage(request.user_id, request.machine_name, request.date, request.domain_summaries)

    return UploadResponse(
        success=True,
        message=f"Received {app_count} apps, {domain_count} domains for {request.user_id} on {request.date}"
    )
```

### Node.js (Express)

```javascript
const express = require('express');
const app = express();

app.use(express.json());

app.post('/api/upload', async (req, res) => {
  const { user_id, machine_name, date, min_duration_seconds, app_summaries, domain_summaries } = req.body;

  if (!user_id) {
    return res.status(400).json({
      success: false,
      message: 'Invalid user_id',
      error_code: 'INVALID_USER'
    });
  }

  const appCount = app_summaries?.length || 0;
  const domainCount = domain_summaries?.length || 0;

  try {
    // TODO: データベースに保存（UPSERT）
    // await upsertAppUsage(user_id, machine_name, date, app_summaries);
    // await upsertDomainUsage(user_id, machine_name, date, domain_summaries);

    res.json({
      success: true,
      message: `Received ${appCount} apps, ${domainCount} domains for ${user_id} on ${date}`
    });
  } catch (error) {
    res.status(500).json({
      success: false,
      message: error.message,
      error_code: 'SERVER_ERROR'
    });
  }
});

app.listen(3000);
```

---

## SAML認証との統合

### ユーザーID照合

デスクトップアプリが送信する `user_id` と、SAML認証で取得する属性を照合します。

| デスクトップ側 | SAML属性（Azure AD） | 照合方法 |
|---------------|---------------------|---------|
| `user@domain.com` (UPN) | `http://schemas.xmlsoap.org/ws/2005/05/identity/claims/upn` | 完全一致 |
| `DOMAIN\user` (SAM) | `http://schemas.microsoft.com/ws/2008/06/identity/claims/windowsaccountname` | 完全一致 |

---

## クライアント設定

デスクトップアプリ側の設定ファイル（`%LOCALAPPDATA%/timetracker/integrations.toml`）:

```toml
[upload]
server_url = "https://timetracker.example.com/api/upload"
enabled = true
auto_upload = false
auto_upload_interval_minutes = 60
min_duration_seconds = 600  # 10分以上使用したアプリ/ドメインのみアップロード
```

| 設定項目 | 型 | デフォルト | 説明 |
|---------|-----|-----------|------|
| `server_url` | string | - | アップロード先エンドポイントURL |
| `enabled` | bool | false | アップロード機能の有効/無効 |
| `auto_upload` | bool | false | 自動アップロードの有効/無効 |
| `auto_upload_interval_minutes` | u32 | 60 | 自動アップロード間隔（分） |
| `min_duration_seconds` | u32 | 600 | 最小使用時間（秒）。この時間以上のデータのみアップロード |

---

## データ例

### アップロードされるデータの例

**設定:** `min_duration_seconds = 600`（10分）

**その日の生データ:**
| アプリ/ドメイン | 合計時間 | アップロード対象 |
|----------------|---------|-----------------|
| Code.exe | 2時間 | ✅ |
| slack.exe | 45分 | ✅ |
| notepad.exe | 3分 | ❌ (10分未満) |
| github.com | 30分 | ✅ |
| google.com | 5分 | ❌ (10分未満) |

**アップロードされるJSON:**
```json
{
  "user_id": "user@domain.com",
  "machine_name": "WORKSTATION01",
  "date": "2024-01-15",
  "min_duration_seconds": 600,
  "app_summaries": [
    { "process_name": "Code.exe", "total_seconds": 7200 },
    { "process_name": "slack.exe", "total_seconds": 2700 }
  ],
  "domain_summaries": [
    { "domain": "github.com", "total_seconds": 1800 }
  ]
}
```

---

## テスト用cURLコマンド

```bash
# 正常なアップロード
curl -X POST http://localhost:3000/api/upload \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "test.user@example.com",
    "machine_name": "TEST-PC",
    "date": "2024-01-15",
    "min_duration_seconds": 600,
    "app_summaries": [
      { "process_name": "Code.exe", "total_seconds": 7200 },
      { "process_name": "slack.exe", "total_seconds": 2700 }
    ],
    "domain_summaries": [
      { "domain": "github.com", "total_seconds": 1800 }
    ]
  }'

# 空のデータ（閾値以上のアプリ/ドメインがない場合）
curl -X POST http://localhost:3000/api/upload \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "test.user@example.com",
    "machine_name": "TEST-PC",
    "date": "2024-01-15",
    "min_duration_seconds": 600,
    "app_summaries": [],
    "domain_summaries": []
  }'
```

---

## Web UI での表示例

サーバー側でデータを受け取った後、SAML認証済みユーザーに対して以下のような表示が可能です：

### 日別サマリー画面
```
2024-01-15 の作業時間

[アプリ使用時間]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Code.exe          ████████████████████  2h 00m
slack.exe         ████████              45m

[ブラウザ閲覧時間]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
github.com        ██████                30m
```

### 週次/月次レポート
- 日ごとの合計作業時間
- よく使うアプリのランキング
- よく閲覧するドメインのランキング
