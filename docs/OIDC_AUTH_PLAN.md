# OIDC認証 実装計画

TimeTrackerデスクトップアプリにOIDC認証を追加するための実装計画書。

## 概要

- **目的**: サーバーへのデータアップロード時に認証を行う
- **認証方式**: OpenID Connect (OIDC)
- **アーキテクチャ**: サーバー経由方式（アプリはサーバーURLのみ保持）
- **IdP**: Keycloak（開発）、Entra ID（本番）※サーバー側で設定

## アーキテクチャ選定

### サーバー経由方式を採用

| | アプリ直接IdP | サーバー経由（採用） |
|---|---|---|
| **アプリの設定** | IdP URL + Client ID | サーバーURLのみ |
| **IdP設定の管理** | アプリごとに設定 | サーバーで一元管理 |
| **IdP変更時** | アプリ更新必要 | サーバー側変更のみ |
| **複数IdP対応** | アプリで切替え | サーバーで制御 |
| **Client Secret** | 使用不可（Public Client） | 使用可（Confidential Client） |

**採用理由:**
1. アプリの設定がシンプル（サーバーURLのみ）
2. IdP設定をサーバー側で一元管理できる
3. Keycloak → Entra ID 移行時にアプリ更新不要
4. サーバー側でConfidential Clientを使えるためセキュア

## 認証フロー

### サーバー経由 Authorization Code Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  TimeTracker    │     │     Server      │     │       IdP       │
│  (Tauri App)    │     │                 │     │ (Keycloak/Entra)│
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
    1. ログインボタン押下        │                       │
         │                       │                       │
    2. ローカルHTTPサーバー起動   │                       │
       (コールバック受信用)       │                       │
         │                       │                       │
    3. ブラウザで開く            │                       │
       GET /auth/login?         │                       │
       redirect_uri=            │                       │
       http://localhost:PORT    │                       │
         │ ─────────────────────>│                       │
         │                       │                       │
         │                       │  4. PKCE生成 +        │
         │                       │     state生成         │
         │                       │                       │
         │  5. 302 Redirect      │                       │
         │     to IdP /authorize │                       │
         │ <─────────────────────│                       │
         │                       │                       │
         │            6. ブラウザがIdPへリダイレクト      │
         │ ──────────────────────────────────────────────>│
         │                       │                       │
         │            7. ユーザーがIdPでログイン          │
         │ <──────────────────────────────────────────────│
         │                       │                       │
         │            8. IdPからサーバーへコールバック    │
         │                       │ <─────────────────────│
         │                       │   ?code=xxx&state=xxx │
         │                       │                       │
         │                       │  9. Token交換         │
         │                       │     (code + PKCE)     │
         │                       │ ─────────────────────>│
         │                       │                       │
         │                       │ 10. Tokens受信        │
         │                       │ <─────────────────────│
         │                       │                       │
         │                       │ 11. セッション/JWT生成 │
         │                       │                       │
         │ 12. アプリへリダイレクト│                       │
         │     ?token=xxx        │                       │
         │ <─────────────────────│                       │
         │                       │                       │
   13. トークン保存              │                       │
       (セキュアストレージ)      │                       │
         │                       │                       │
   14. API呼び出し時             │                       │
       Authorization: Bearer    │                       │
         │ ─────────────────────>│                       │
```

### フローの詳細

1. **アプリ**: ユーザーがログインボタンをクリック
2. **アプリ**: ローカルHTTPサーバーを起動（コールバック受信用）
3. **アプリ**: ブラウザでサーバーの `/auth/login` を開く
4. **サーバー**: PKCE（code_verifier, code_challenge）とstateを生成、セッションに保存
5. **サーバー**: IdPの認証エンドポイントへリダイレクト
6. **ブラウザ**: IdPの認証画面へ遷移
7. **ユーザー**: IdPでログイン（ID/パスワード、MFA等）
8. **IdP**: 認証成功後、サーバーのコールバックURLへリダイレクト
9. **サーバー**: Authorization CodeをIdPのトークンエンドポイントに送信
10. **サーバー**: ID Token + Access Token + Refresh Token を受信
11. **サーバー**: アプリ用のトークン（またはセッション）を生成
12. **サーバー**: アプリのローカルサーバーへリダイレクト（トークン付与）
13. **アプリ**: トークンをセキュアストレージに保存
14. **アプリ**: 以降のAPI呼び出しでBearerトークンを使用

## トークンの使い分け

### サーバーが発行するトークン

サーバー経由方式では、IdPから受け取ったトークンをそのままアプリに渡すか、サーバー独自のトークンを発行するか選択できる。

| 方式 | メリット | デメリット |
|------|---------|-----------|
| **IdPトークン転送** | シンプル | トークン有効期限がIdP依存 |
| **サーバー独自JWT** | 柔軟な制御 | サーバー側実装が必要 |

**推奨: サーバー独自JWT または IdPのAccess Token転送**

### Access Token vs ID Token（参考）

| | Access Token | ID Token |
|---|---|---|
| **目的** | 認可（Authorization） | 認証（Authentication） |
| **用途** | API呼び出しに使用 | ユーザー情報の確認 |
| **推奨** | API認可にはこちら | ログイン完了の証明 |

## 設定

### アプリ側の設定（シンプル）

```toml
# integrations.toml
[server]
url = "https://api.example.com"

# IdP設定は不要！サーバーが管理する
```

### サーバー側の設定（IdP情報を保持）

```yaml
# 環境変数 または 設定ファイル

# 開発環境 (Keycloak)
OIDC_ISSUER_URL=https://keycloak.example.com/realms/your-realm
OIDC_CLIENT_ID=timetracker-server
OIDC_CLIENT_SECRET=your-client-secret
OIDC_SCOPES=openid,profile,email,offline_access

# 本番環境 (Entra ID)
OIDC_ISSUER_URL=https://login.microsoftonline.com/{tenant-id}/v2.0
OIDC_CLIENT_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
OIDC_CLIENT_SECRET=your-client-secret
OIDC_SCOPES=openid,profile,email,offline_access
```

### IdP側の設定

#### Keycloak（開発環境）

- Client Type: `confidential`（サーバーがsecretを保持）
- Valid Redirect URIs: `https://api.example.com/auth/callback`
- PKCE: 有効化推奨（追加のセキュリティ）

#### Entra ID（本番環境）

- アプリケーションの種類: Web アプリケーション
- リダイレクトURI: `https://api.example.com/auth/callback`
- クライアントシークレット: 生成して設定
- APIのアクセス許可: `openid`, `profile`, `email`, `offline_access`

## サーバー側 必要なエンドポイント

```
GET  /auth/login
     - クエリパラメータ: redirect_uri (アプリのローカルサーバーURL)
     - 処理: PKCE生成、stateをセッションに保存、IdPへリダイレクト

GET  /auth/callback
     - IdPからのコールバック受信
     - 処理: code検証、トークン交換、アプリへリダイレクト

POST /auth/refresh
     - リクエスト: refresh_token
     - 処理: トークンリフレッシュ
     - レスポンス: 新しいaccess_token

GET  /auth/userinfo
     - ヘッダー: Authorization: Bearer {token}
     - レスポンス: ユーザー情報（sub, email, name等）

POST /auth/logout
     - 処理: トークン無効化（オプション）
```

### エンドポイント詳細

#### GET /auth/login

```
リクエスト:
  GET /auth/login?redirect_uri=http://localhost:8400/callback

レスポンス:
  302 Found
  Location: https://idp.example.com/authorize?
    client_id=xxx&
    redirect_uri=https://api.example.com/auth/callback&
    response_type=code&
    scope=openid+profile+email&
    state=xxx&
    code_challenge=xxx&
    code_challenge_method=S256&
    login_hint=xxx  (オプション)
```

#### GET /auth/callback

```
リクエスト (IdPから):
  GET /auth/callback?code=xxx&state=xxx

処理:
  1. state検証（CSRF対策）
  2. IdPトークンエンドポイントでcode交換
  3. ユーザー情報取得/検証
  4. アプリ用トークン生成

レスポンス:
  302 Found
  Location: http://localhost:8400/callback?
    access_token=xxx&
    refresh_token=xxx&
    expires_in=3600
```

## アプリ側 実装コンポーネント

### Rust依存関係（簡素化）

`app/src-tauri/Cargo.toml`:

```toml
[dependencies]
# 既存の依存関係に加えて
keyring = "2"            # OSセキュアストレージ（Windows Credential Manager）
# oauth2/openidconnect は不要（サーバーが処理）
```

### モジュール構成（簡素化）

```
app/src-tauri/src/
├── auth/
│   ├── mod.rs           # 認証モジュール エントリポイント
│   ├── token_store.rs   # トークン保存・取得
│   │                    # - Windows Credential Manager連携
│   │                    # - トークンのシリアライズ/デシリアライズ
│   └── local_server.rs  # リダイレクト受信用ローカルサーバー
│                        # - 空きポート検出
│                        # - コールバック処理
│                        # - トークン抽出
└── lib.rs               # Tauriコマンド追加
```

### Tauriコマンド（API）

```rust
/// ログイン開始 - ブラウザを開いてサーバー経由でOIDCログインを開始
#[tauri::command]
async fn start_login(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // 1. ローカルサーバー起動
    let (port, receiver) = start_callback_server()?;
    let redirect_uri = format!("http://localhost:{}/callback", port);

    // 2. ブラウザでサーバーの認証エンドポイントを開く
    let server_url = state.config.server_url;
    let auth_url = format!("{}/auth/login?redirect_uri={}", server_url, redirect_uri);
    open::that(auth_url)?;

    // 3. コールバックを待機してトークンを取得
    let tokens = receiver.await?;

    // 4. トークンを保存
    save_tokens(&tokens)?;

    Ok(())
}

/// 認証状態確認
#[tauri::command]
fn is_authenticated(state: State<'_, Arc<AppState>>) -> bool {
    load_tokens().map(|t| t.is_some() && !t.unwrap().is_expired()).unwrap_or(false)
}

/// ログアウト（トークン削除）
#[tauri::command]
fn logout(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    delete_tokens()
}

/// 現在のユーザー情報取得（サーバーから取得）
#[tauri::command]
async fn get_current_user(state: State<'_, Arc<AppState>>) -> Result<UserInfo, String> {
    let token = get_valid_access_token(&state).await?;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/auth/userinfo", state.config.server_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;
    response.json::<UserInfo>().await.map_err(|e| e.to_string())
}

/// トークンリフレッシュ
#[tauri::command]
async fn refresh_token(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let tokens = load_tokens()?.ok_or("Not authenticated")?;
    let refresh_token = tokens.refresh_token.ok_or("No refresh token")?;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/auth/refresh", state.config.server_url))
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await?;

    let new_tokens: TokenSet = response.json().await?;
    save_tokens(&new_tokens)?;
    Ok(())
}
```

### トークン保存（セキュアストレージ）

Windows Credential Managerを使用して安全にトークンを保存:

```rust
use keyring::Entry;

const SERVICE_NAME: &str = "timetracker";
const TOKEN_KEY: &str = "auth_tokens";

#[derive(Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
}

impl TokenSet {
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }

    pub fn is_expiring_soon(&self) -> bool {
        self.expires_at <= Utc::now() + Duration::seconds(30)
    }
}

pub fn save_tokens(tokens: &TokenSet) -> Result<(), Error> {
    let entry = Entry::new(SERVICE_NAME, TOKEN_KEY)?;
    let json = serde_json::to_string(tokens)?;
    entry.set_password(&json)?;
    Ok(())
}

pub fn load_tokens() -> Result<Option<TokenSet>, Error> {
    let entry = Entry::new(SERVICE_NAME, TOKEN_KEY)?;
    match entry.get_password() {
        Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_tokens() -> Result<(), Error> {
    let entry = Entry::new(SERVICE_NAME, TOKEN_KEY)?;
    entry.delete_credential()?;
    Ok(())
}
```

### トークン自動リフレッシュ

```rust
/// 有効なAccess Tokenを取得（必要に応じて自動リフレッシュ）
async fn get_valid_access_token(state: &AppState) -> Result<String, AuthError> {
    let tokens = load_tokens()?.ok_or(AuthError::NotAuthenticated)?;

    if !tokens.is_expiring_soon() {
        // まだ有効
        return Ok(tokens.access_token);
    }

    // Access Token期限切れ → サーバーでリフレッシュ
    if let Some(refresh_token) = &tokens.refresh_token {
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/auth/refresh", state.config.server_url))
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let new_tokens: TokenSet = resp.json().await?;
                save_tokens(&new_tokens)?;
                return Ok(new_tokens.access_token);
            }
            _ => {
                // Refresh Token も無効 → 再ログイン必要
                delete_tokens()?;
                return Err(AuthError::RequiresReauth);
            }
        }
    }

    Err(AuthError::RequiresReauth)
}
```

### ローカルコールバックサーバー

```rust
use std::net::TcpListener;
use std::io::{Read, Write};

/// 空きポートを見つけてローカルサーバーを起動
pub fn start_callback_server() -> Result<(u16, oneshot::Receiver<TokenSet>), Error> {
    // 空きポートを探す
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();

    let (tx, rx) = oneshot::channel();

    // 別スレッドでコールバックを待機
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0; 4096];
            if let Ok(size) = stream.read(&mut buffer) {
                let request = String::from_utf8_lossy(&buffer[..size]);

                // URLからトークンを抽出
                if let Some(tokens) = parse_callback_tokens(&request) {
                    // 成功レスポンスを返す
                    let response = "HTTP/1.1 200 OK\r\n\
                        Content-Type: text/html\r\n\r\n\
                        <html><body><h1>ログイン成功</h1>\
                        <p>このウィンドウを閉じてアプリに戻ってください。</p>\
                        <script>window.close();</script></body></html>";
                    let _ = stream.write_all(response.as_bytes());

                    let _ = tx.send(tokens);
                }
            }
        }
    });

    Ok((port, rx))
}

fn parse_callback_tokens(request: &str) -> Option<TokenSet> {
    // GET /callback?access_token=xxx&refresh_token=xxx&expires_in=3600 HTTP/1.1
    let url_part = request.lines().next()?.split_whitespace().nth(1)?;
    let query = url_part.split('?').nth(1)?;

    let mut access_token = None;
    let mut refresh_token = None;
    let mut expires_in = 3600i64;

    for param in query.split('&') {
        let mut parts = param.split('=');
        match (parts.next(), parts.next()) {
            (Some("access_token"), Some(v)) => access_token = Some(v.to_string()),
            (Some("refresh_token"), Some(v)) => refresh_token = Some(v.to_string()),
            (Some("expires_in"), Some(v)) => expires_in = v.parse().unwrap_or(3600),
            _ => {}
        }
    }

    Some(TokenSet {
        access_token: access_token?,
        refresh_token,
        expires_at: Utc::now() + Duration::seconds(expires_in),
    })
}
```

### アップロード機能との統合

既存の `upload_aggregated_data` を認証対応に:

```rust
async fn upload_aggregated_data(
    state: State<'_, Arc<AppState>>,
    // ...
) -> Result<UploadResponse, String> {
    // 有効なAccess Tokenを取得（自動リフレッシュ込み）
    let access_token = get_valid_access_token(&state)
        .await
        .map_err(|e| match e {
            AuthError::RequiresReauth => "再ログインが必要です".to_string(),
            AuthError::NotAuthenticated => "ログインしてください".to_string(),
            _ => e.to_string(),
        })?;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/upload", state.config.server_url))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // レスポンス処理...
}
```

### フロントエンド変更

`app/src/main.ts`:

```typescript
interface UserInfo {
  sub: string;
  email?: string;
  name?: string;
}

// 認証状態の確認
async function checkAuth(): Promise<boolean> {
  return await invoke<boolean>("is_authenticated");
}

// ログイン開始
async function login(): Promise<void> {
  try {
    await invoke("start_login");
    // ログイン完了後、UIを更新
    const user = await getCurrentUser();
    updateAuthUI(true, user);
  } catch (e) {
    console.error("Login failed:", e);
  }
}

// ログアウト
async function logout(): Promise<void> {
  await invoke("logout");
  updateAuthUI(false);
}

// 現在のユーザー情報取得
async function getCurrentUser(): Promise<UserInfo | null> {
  if (await checkAuth()) {
    return await invoke<UserInfo>("get_current_user");
  }
  return null;
}

// UI更新
function updateAuthUI(authenticated: boolean, user?: UserInfo | null): void {
  const authSection = document.getElementById("auth-section");
  if (!authSection) return;

  if (authenticated && user) {
    authSection.innerHTML = `
      <span class="user-info">${user.email || user.name || user.sub}</span>
      <button class="auth-button" onclick="logout()">ログアウト</button>
    `;
  } else {
    authSection.innerHTML = `
      <button class="auth-button" onclick="login()">ログイン</button>
    `;
  }
}

// アプリ起動時に認証状態をチェック
async function initAuth(): Promise<void> {
  const authenticated = await checkAuth();
  if (authenticated) {
    const user = await getCurrentUser();
    updateAuthUI(true, user);
  } else {
    updateAuthUI(false);
  }
}

// DOMContentLoaded時に実行
document.addEventListener("DOMContentLoaded", initAuth);
```

## 実装フェーズ

### Phase 1: サーバー側エンドポイント（優先度: 高）

- [ ] `GET /auth/login` - 認証開始、IdPへリダイレクト
- [ ] `GET /auth/callback` - IdPコールバック処理、トークン交換
- [ ] `POST /auth/refresh` - トークンリフレッシュ
- [ ] `GET /auth/userinfo` - ユーザー情報取得

### Phase 2: アプリ側基盤（優先度: 高）

- [ ] 設定ファイル構造定義（`[server]` セクション）
- [ ] `keyring` クレート追加
- [ ] ローカルHTTPサーバー実装（コールバック受信）

### Phase 3: アプリ側トークン管理（優先度: 高）

- [ ] Windows Credential Manager連携
- [ ] トークン保存・取得・削除
- [ ] 有効期限チェック
- [ ] 自動リフレッシュ

### Phase 4: Tauriコマンド（優先度: 中）

- [ ] `start_login` コマンド
- [ ] `is_authenticated` コマンド
- [ ] `logout` コマンド
- [ ] `get_current_user` コマンド
- [ ] `refresh_token` コマンド

### Phase 5: フロントエンドUI（優先度: 中）

- [ ] ログイン/ログアウトボタン
- [ ] 認証状態表示
- [ ] ユーザー情報表示

### Phase 6: 統合（優先度: 中）

- [ ] アップロード機能にBearer認証追加
- [ ] 認証エラー時の再ログインフロー
- [ ] 401レスポンス時の自動リフレッシュ

### Phase 7: テスト・改善（優先度: 低）

- [ ] Keycloakでのテスト
- [ ] Entra IDでのテスト
- [ ] エラーハンドリング改善
- [ ] ログ出力

## セキュリティ考慮事項

### サーバー側

| 対策 | 説明 |
|------|------|
| **Confidential Client** | サーバーがclient_secretを安全に保持 |
| **PKCE** | 追加のセキュリティ層として使用推奨 |
| **State パラメータ** | CSRF対策。セッションに保存して検証 |
| **HTTPS必須** | 本番環境では全通信をHTTPS化 |
| **トークン検証** | IdPから受け取ったトークンの署名を検証 |

### アプリ側

| 対策 | 説明 |
|------|------|
| **セキュアストレージ** | トークンはOS標準のCredential Manager使用 |
| **ローカルサーバー** | 127.0.0.1のみバインド、外部からアクセス不可 |
| **トークン有効期限** | 期限切れトークンは自動削除 |
| **ログアウト** | トークンを確実に削除 |

### トークン管理

- サーバー側でトークンの有効期限を適切に設定
- Refresh Tokenは必要に応じてローテーション
- アプリ終了時もトークンは保持（次回起動時に再利用）
- 401レスポンス時は自動リフレッシュを試行

## IdP別の注意点

### Keycloak（開発環境）

```json
// ID Tokenのクレーム例
{
  "sub": "user-uuid",
  "preferred_username": "tanaka",
  "email": "tanaka@example.com",
  "name": "田中 太郎"
}
```

- トークン有効期限はRealm設定で変更可能
- Client Scopeでクレームをカスタマイズ可能

### Entra ID（本番環境）

```json
// ID Tokenのクレーム例
{
  "sub": "xxxx",
  "preferred_username": "tanaka@company.com",
  "email": "tanaka@company.com",
  "name": "田中 太郎",
  "oid": "user-object-id",
  "tid": "tenant-id"
}
```

- `oid`（Object ID）が一意のユーザー識別子
- 条件付きアクセスポリシーとの連携が可能
- グループメンバーシップをクレームに含めることが可能

**ユーザー識別には `sub` または `email` を使用すれば両方のIdPで動作する。**

## 参考リンク

- [OAuth 2.0 for Native Apps (RFC 8252)](https://datatracker.ietf.org/doc/html/rfc8252)
- [PKCE (RFC 7636)](https://datatracker.ietf.org/doc/html/rfc7636)
- [OpenID Connect Core](https://openid.net/specs/openid-connect-core-1_0.html)
- [Keycloak Documentation](https://www.keycloak.org/documentation)
- [Microsoft Entra ID Documentation](https://learn.microsoft.com/en-us/entra/identity-platform/)
