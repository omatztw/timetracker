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
│                                    │  Database        │        │
│  ┌──────────────┐     SAML        │  (user_id で紐付け)│        │
│  │  ブラウザ     │ ←────────────→ └──────────────────┘        │
│  │              │                          │                  │
│  └──────────────┘                          ▼                  │
│         │                          ┌──────────────────┐        │
│         └─────────────────────────→│  Web UI          │        │
│              閲覧（自分のデータ）    │  (SAML認証済み)   │        │
│                                    └──────────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

## API Endpoints

### POST /api/upload

デスクトップアプリからアクティビティデータを受け取ります。

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
  "activities": [
    {
      "id": 12345,
      "process_name": "chrome.exe",
      "window_title": "GitHub - Project",
      "domain": "github.com",
      "start_time": "2024-01-15T09:30:00",
      "end_time": "2024-01-15T09:45:00",
      "duration_seconds": 900
    },
    {
      "id": 12346,
      "process_name": "Code.exe",
      "window_title": "main.ts - timetracker",
      "domain": null,
      "start_time": "2024-01-15T09:45:00",
      "end_time": "2024-01-15T10:30:00",
      "duration_seconds": 2700
    }
  ]
}
```

#### Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `user_id` | string | Yes | Windowsログインユーザー名（UPN形式: `user@domain.com` または SAM形式: `DOMAIN\user`） |
| `machine_name` | string | No | マシン名（`COMPUTERNAME` 環境変数） |
| `activities` | array | Yes | アクティビティレコードの配列 |

#### Activity Record Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | integer | Yes | クライアント側のローカルID（重複検知に使用可能） |
| `process_name` | string | Yes | プロセス名（例: `chrome.exe`, `Code.exe`） |
| `window_title` | string | Yes | ウィンドウタイトル |
| `domain` | string | No | ブラウザの場合のドメイン（`null` for non-browser apps） |
| `start_time` | string | Yes | 開始時刻（ISO 8601形式: `YYYY-MM-DDTHH:MM:SS`） |
| `end_time` | string | Yes | 終了時刻（ISO 8601形式: `YYYY-MM-DDTHH:MM:SS`） |
| `duration_seconds` | integer | Yes | 継続時間（秒） |

#### Response

**Success (200 OK):**
```json
{
  "success": true,
  "message": "Uploaded 15 activities",
  "received_count": 15
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
| `INVALID_DATA` | 400 | activities データが不正 |
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

-- アクティビティテーブル
CREATE TABLE activities (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    machine_name VARCHAR(255),
    client_id BIGINT,                       -- クライアント側のID
    process_name VARCHAR(255) NOT NULL,
    window_title TEXT NOT NULL,
    domain VARCHAR(255),
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    duration_seconds INTEGER NOT NULL,
    uploaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    -- インデックス
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id)
);

-- パフォーマンス用インデックス
CREATE INDEX idx_activities_user_id ON activities(user_id);
CREATE INDEX idx_activities_start_time ON activities(start_time);
CREATE INDEX idx_activities_user_date ON activities(user_id, start_time);

-- 重複防止用ユニーク制約（オプション）
CREATE UNIQUE INDEX idx_activities_unique
ON activities(user_id, machine_name, client_id);
```

---

## サーバー実装例

### Python (FastAPI)

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import Optional, List
from datetime import datetime

app = FastAPI()

class Activity(BaseModel):
    id: int
    process_name: str
    window_title: str
    domain: Optional[str]
    start_time: str
    end_time: str
    duration_seconds: int

class UploadRequest(BaseModel):
    user_id: str
    machine_name: Optional[str]
    activities: List[Activity]

class UploadResponse(BaseModel):
    success: bool
    message: str
    received_count: int = 0

@app.post("/api/upload", response_model=UploadResponse)
async def upload_activities(request: UploadRequest):
    if not request.user_id:
        raise HTTPException(status_code=400, detail="Invalid user_id")

    if not request.activities:
        return UploadResponse(
            success=True,
            message="No activities to upload",
            received_count=0
        )

    # TODO: データベースに保存
    # await save_activities(request.user_id, request.machine_name, request.activities)

    return UploadResponse(
        success=True,
        message=f"Uploaded {len(request.activities)} activities",
        received_count=len(request.activities)
    )
```

### Node.js (Express)

```javascript
const express = require('express');
const app = express();

app.use(express.json());

app.post('/api/upload', async (req, res) => {
  const { user_id, machine_name, activities } = req.body;

  if (!user_id) {
    return res.status(400).json({
      success: false,
      message: 'Invalid user_id',
      error_code: 'INVALID_USER'
    });
  }

  if (!activities || activities.length === 0) {
    return res.json({
      success: true,
      message: 'No activities to upload',
      received_count: 0
    });
  }

  try {
    // TODO: データベースに保存
    // await saveActivities(user_id, machine_name, activities);

    res.json({
      success: true,
      message: `Uploaded ${activities.length} activities`,
      received_count: activities.length
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

### SAML SP設定例（Azure AD）

1. **Azure AD Enterprise Application** を作成
2. **SAML SSO** を設定
3. **属性マッピング**:
   - `user.userprincipalname` → `upn`
   - `user.onpremisessamaccountname` → `samAccountName`

---

## セキュリティ考慮事項

### 現在の実装（社内利用前提）

- ユーザー名のみで認証（トークン検証なし）
- 社内ネットワークからのアクセスを想定

### 推奨セキュリティ対策

1. **ネットワーク制限**: VPN/社内LANからのみアクセス可能に
2. **レート制限**: 同一ユーザーからの過度なリクエストを制限
3. **入力検証**: user_id のフォーマット検証
4. **ログ記録**: アップロード元IPとuser_idを記録

### 将来の強化オプション

WAM（Web Account Manager）を使用したトークン認証に移行する場合:

```python
# サーバー側でAzure ADトークンを検証
from azure.identity import DefaultAzureCredential
import jwt

def verify_token(token: str) -> dict:
    # Azure AD公開鍵でトークンを検証
    # ...
    return decoded_claims
```

---

## クライアント設定

デスクトップアプリ側の設定ファイル（`%LOCALAPPDATA%/timetracker/integrations.toml`）:

```toml
[upload]
server_url = "https://timetracker.example.com/api/upload"
enabled = true
auto_upload = false
auto_upload_interval_minutes = 60
```

| 設定項目 | 型 | デフォルト | 説明 |
|---------|-----|-----------|------|
| `server_url` | string | - | アップロード先エンドポイントURL |
| `enabled` | bool | false | アップロード機能の有効/無効 |
| `auto_upload` | bool | false | 自動アップロードの有効/無効 |
| `auto_upload_interval_minutes` | u32 | 60 | 自動アップロード間隔（分） |

---

## テスト用cURLコマンド

```bash
# 正常なアップロード
curl -X POST http://localhost:3000/api/upload \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "test.user@example.com",
    "machine_name": "TEST-PC",
    "activities": [
      {
        "id": 1,
        "process_name": "chrome.exe",
        "window_title": "Test Page",
        "domain": "example.com",
        "start_time": "2024-01-15T09:00:00",
        "end_time": "2024-01-15T09:30:00",
        "duration_seconds": 1800
      }
    ]
  }'

# 空のアクティビティ
curl -X POST http://localhost:3000/api/upload \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "test.user@example.com",
    "machine_name": "TEST-PC",
    "activities": []
  }'
```
