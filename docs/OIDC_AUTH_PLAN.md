# OIDC認証 実装計画

TimeTrackerデスクトップアプリにOIDC認証を追加するための実装計画書。

## 概要

- **目的**: サーバーへのデータアップロード時に認証を行う
- **認証方式**: OpenID Connect (OIDC)
- **フロー**: Authorization Code Flow with PKCE
- **IdP**: Keycloak（開発）、Entra ID（本番）

## 認証フロー

### Authorization Code Flow with PKCE

デスクトップアプリ（Public Client）に最適な認証フローを採用する。

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  TimeTracker    │     │    ブラウザ       │     │  OIDC Provider  │
│  (Tauri App)    │     │                  │     │  (IdP)          │
└────────┬────────┘     └────────┬─────────┘     └────────┬────────┘
         │                       │                        │
    1. ログインボタン押下        │                        │
         │                       │                        │
    2. PKCE生成                  │                        │
       (code_verifier,          │                        │
        code_challenge)         │                        │
         │                       │                        │
    3. ローカルHTTPサーバー起動   │                        │
       (リダイレクト受信用)       │                        │
         │                       │                        │
    4. ブラウザを開く ──────────>│                        │
       /authorize?              │                        │
       client_id=xxx&           │   5. 認証リクエスト     │
       redirect_uri=            │ ─────────────────────> │
       http://localhost:PORT&   │                        │
       code_challenge=xxx&      │   6. ユーザーログイン   │
       response_type=code&      │ <───────────────────── │
       state=xxx                │                        │
         │                       │   7. Authorization Code │
         │   8. リダイレクト     │ <───────────────────── │
         │ <────────────────────│                        │
         │   ?code=xxx&state=xxx│                        │
         │                       │                        │
    9. Token Request ─────────────────────────────────> │
       (code + code_verifier)   │                        │
         │                       │                        │
   10. ID Token + Access Token <─────────────────────── │
       + Refresh Token          │                        │
         │                       │                        │
   11. トークン保存              │                        │
       (セキュアストレージ)      │                        │
         │                       │                        │
   12. API呼び出し時             │                        │
       Authorization: Bearer    │                        │
```

### なぜPKCEが必要か

- デスクトップアプリは `client_secret` を安全に保持できない（バイナリ解析で漏洩リスク）
- PKCE（Proof Key for Code Exchange）により、Authorization Codeの横取り攻撃を防止
- Keycloak、Entra ID ともにPKCE対応済み

## トークンの使い分け

### Access Token vs ID Token

| | Access Token | ID Token |
|---|---|---|
| **目的** | 認可（Authorization）リソースへのアクセス権 | 認証（Authentication）ユーザーが誰か |
| **形式** | JWT or Opaque（実装依存） | 必ずJWT |
| **送信先** | リソースサーバー（API） | クライアント側で消費 |
| **中身** | スコープ（権限）情報 | ユーザー情報（sub, email, name等） |
| **仕様上の用途** | API呼び出しに使う | ログイン完了の証明 |

### 推奨: Access Token を使用

```
Client → API: Authorization: Bearer {access_token}
```

**理由:**
1. OIDC仕様に沿っている（ID Tokenは認証の証明、API認可にはAccess Token）
2. ID Tokenにはユーザー情報が含まれるため、毎回送信すると情報漏洩リスク
3. Access Tokenはスコープで権限を細かく制御できる

### Refresh Token

- Access Token / ID Token の有効期限は短い（数分〜1時間）
- ユーザーに再ログインさせずにトークンを更新するために使用
- `offline_access` スコープを要求して取得

## IdP別の設定

### Keycloak（開発環境）

```toml
[auth]
enabled = true
provider = "oidc"

[auth.oidc]
issuer_url = "https://keycloak.example.com/realms/your-realm"
client_id = "timetracker-desktop"
scopes = ["openid", "profile", "email", "offline_access"]
```

**Keycloak側の設定:**
- Client Type: `public`（Confidentialではない）
- Valid Redirect URIs: `http://localhost:*`
- PKCE: 有効化推奨

### Entra ID（本番環境）

```toml
[auth]
enabled = true
provider = "oidc"

[auth.oidc]
issuer_url = "https://login.microsoftonline.com/{tenant-id}/v2.0"
client_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
scopes = ["openid", "profile", "email", "offline_access"]
```

**Entra ID側の設定（アプリ登録）:**
- アプリケーションの種類: パブリック クライアント / ネイティブ
- リダイレクトURI: `http://localhost:8400/callback` など（複数ポート登録推奨）
- APIのアクセス許可: `openid`, `profile`, `email`, `offline_access`

### クレーム名の違い

```json
// Keycloak (標準的)
{
  "sub": "user-uuid",
  "preferred_username": "tanaka",
  "email": "tanaka@example.com"
}

// Entra ID (独自クレームあり)
{
  "sub": "xxxx",
  "preferred_username": "tanaka@company.com",
  "email": "tanaka@company.com",
  "oid": "user-object-id",
  "tid": "tenant-id"
}
```

**ユーザー識別には `sub` または `email` を使用すれば両方のIdPで動作する。**

### トークン有効期限（デフォルト）

| | Keycloak | Entra ID |
|---|---|---|
| Access Token | 5分（設定変更可） | 60-90分 |
| Refresh Token | 30日（設定変更可） | 90日（設定変更可） |

## 実装コンポーネント

### Rust依存関係の追加

`app/src-tauri/Cargo.toml`:

```toml
[dependencies]
oauth2 = "4.4"           # OAuth2/OIDC フロー実装
openidconnect = "3.5"    # OIDC固有機能（Discovery、ID Token検証等）
keyring = "2"            # OSセキュアストレージ（Windows Credential Manager）
base64 = "0.22"          # PKCEエンコーディング
rand = "0.8"             # PKCE code_verifier生成
```

### モジュール構成

```
app/src-tauri/src/
├── auth/
│   ├── mod.rs           # 認証モジュール エントリポイント
│   ├── oidc.rs          # OIDCフロー実装
│   │                    # - Discovery document取得
│   │                    # - 認証URL生成
│   │                    # - トークン取得
│   │                    # - トークンリフレッシュ
│   ├── token_store.rs   # トークン保存・取得
│   │                    # - Windows Credential Manager連携
│   │                    # - トークンのシリアライズ/デシリアライズ
│   ├── pkce.rs          # PKCE生成ヘルパー
│   │                    # - code_verifier生成
│   │                    # - code_challenge計算（S256）
│   └── local_server.rs  # リダイレクト受信用ローカルサーバー
│                        # - 空きポート検出
│                        # - コールバック処理
│                        # - 認証コード抽出
└── lib.rs               # Tauriコマンド追加
```

### Tauriコマンド（API）

```rust
/// ログイン開始 - ブラウザを開いてOIDCログインを開始
#[tauri::command]
async fn start_oidc_login(state: State<'_, Arc<AppState>>) -> Result<(), String>

/// 認証状態確認
#[tauri::command]
fn is_authenticated(state: State<'_, Arc<AppState>>) -> bool

/// ログアウト（トークン削除）
#[tauri::command]
fn logout(state: State<'_, Arc<AppState>>) -> Result<(), String>

/// 現在のユーザー情報取得（IDトークンのclaims）
#[tauri::command]
fn get_current_user(state: State<'_, Arc<AppState>>) -> Result<UserInfo, String>

/// トークンリフレッシュ（通常は内部で自動呼び出し）
#[tauri::command]
async fn refresh_token(state: State<'_, Arc<AppState>>) -> Result<(), String>
```

### トークン保存（セキュアストレージ）

Windows Credential Managerを使用して安全にトークンを保存:

```rust
use keyring::Entry;

const SERVICE_NAME: &str = "timetracker";
const TOKEN_KEY: &str = "oidc_tokens";

#[derive(Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
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
```

### トークンリフレッシュ戦略

```rust
/// 有効なAccess Tokenを取得（必要に応じて自動リフレッシュ）
async fn get_valid_access_token(&self) -> Result<String, AuthError> {
    let tokens = self.load_tokens()?;

    // 有効期限の30秒前にリフレッシュ（バッファ）
    let buffer = Duration::seconds(30);

    if tokens.expires_at > Utc::now() + buffer {
        // まだ有効
        return Ok(tokens.access_token);
    }

    // Access Token期限切れ → Refresh Tokenで更新
    if let Some(refresh_token) = &tokens.refresh_token {
        match self.refresh_tokens(refresh_token).await {
            Ok(new_tokens) => {
                self.save_tokens(&new_tokens)?;
                return Ok(new_tokens.access_token);
            }
            Err(_) => {
                // Refresh Tokenも無効 → 再ログイン必要
                return Err(AuthError::RequiresReauth);
            }
        }
    }

    Err(AuthError::RequiresReauth)
}
```

### リダイレクト受信方式

**ローカルHTTPサーバー方式（推奨）:**

```rust
use std::net::TcpListener;

/// 空きポートを見つけてローカルサーバーを起動
fn start_callback_server() -> Result<(u16, oneshot::Receiver<String>), Error> {
    // 空きポートを探す
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();

    let (tx, rx) = oneshot::channel();

    // 別スレッドでコールバックを待機
    std::thread::spawn(move || {
        // HTTPリクエストを受信してauthorization codeを抽出
        // txに送信
    });

    Ok((port, rx))
}
```

**リダイレクトURI:** `http://localhost:{PORT}/callback`

### アップロード機能との統合

既存の `upload_aggregated_data` を認証対応に:

```rust
async fn upload_aggregated_data(
    state: State<'_, Arc<AppState>>,
    // ...
) -> Result<UploadResponse, String> {
    // 有効なAccess Tokenを取得（自動リフレッシュ込み）
    let access_token = state.auth
        .get_valid_access_token()
        .await
        .map_err(|e| match e {
            AuthError::RequiresReauth => "再ログインが必要です".to_string(),
            _ => e.to_string(),
        })?;

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
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
  await invoke("start_oidc_login");
  // ブラウザが開く → ユーザーがログイン
  // 完了後、イベントまたはポーリングで検知
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
function updateAuthUI(authenticated: boolean, user?: UserInfo): void {
  const authSection = document.getElementById("auth-section");
  if (authenticated && user) {
    authSection.innerHTML = `
      <span>${user.email || user.name || user.sub}</span>
      <button onclick="logout()">ログアウト</button>
    `;
  } else {
    authSection.innerHTML = `
      <button onclick="login()">ログイン</button>
    `;
  }
}
```

## 実装フェーズ

### Phase 1: 基盤（優先度: 高）

- [ ] `oauth2`/`openidconnect` クレート追加
- [ ] 設定ファイル構造定義（`integrations.toml` に `[auth]` セクション追加）
- [ ] PKCEユーティリティ実装

### Phase 2: 認証フロー（優先度: 高）

- [ ] OIDC Discovery実装
- [ ] ローカルHTTPサーバー実装（コールバック受信）
- [ ] ブラウザ起動 + 認証URL生成
- [ ] Authorization Code → Token交換

### Phase 3: トークン管理（優先度: 高）

- [ ] Windows Credential Manager連携（`keyring`クレート）
- [ ] トークン保存・取得
- [ ] トークンリフレッシュ実装
- [ ] 有効期限チェック

### Phase 4: Tauriコマンド（優先度: 中）

- [ ] `start_oidc_login` コマンド
- [ ] `is_authenticated` コマンド
- [ ] `logout` コマンド
- [ ] `get_current_user` コマンド

### Phase 5: フロントエンドUI（優先度: 中）

- [ ] ログイン/ログアウトボタン
- [ ] 認証状態表示
- [ ] ユーザー情報表示

### Phase 6: 統合（優先度: 中）

- [ ] アップロード機能にBearer認証追加
- [ ] 認証エラー時の再ログインフロー
- [ ] 自動リフレッシュ

### Phase 7: テスト・改善（優先度: 低）

- [ ] Keycloakでのテスト
- [ ] Entra IDでのテスト
- [ ] エラーハンドリング改善
- [ ] ログ出力

## セキュリティ考慮事項

### 必須対策

| 対策 | 説明 |
|------|------|
| **PKCE** | Public Clientのためclient_secretなし。PKCEで認証コード横取りを防止 |
| **State パラメータ** | CSRF対策。認証リクエストとコールバックの紐付け |
| **nonce** | IDトークンのリプレイ攻撃対策 |
| **セキュアストレージ** | トークンは平文ファイルではなくOS標準のCredential Manager使用 |
| **HTTPS** | 本番環境ではHTTPS必須（localhostは例外） |

### トークン管理

- Access Tokenは有効期限を短く設定（IdP側）
- Refresh Tokenは必要に応じてローテーション（IdP側で設定）
- ログアウト時はトークンを確実に削除
- アプリ終了時もトークンは保持（次回起動時に再利用）

## 参考リンク

- [OAuth 2.0 for Native Apps (RFC 8252)](https://datatracker.ietf.org/doc/html/rfc8252)
- [PKCE (RFC 7636)](https://datatracker.ietf.org/doc/html/rfc7636)
- [OpenID Connect Core](https://openid.net/specs/openid-connect-core-1_0.html)
- [openidconnect-rs](https://github.com/ramosbugs/openidconnect-rs)
- [Keycloak Documentation](https://www.keycloak.org/documentation)
- [Microsoft Entra ID Documentation](https://learn.microsoft.com/en-us/entra/identity-platform/)
