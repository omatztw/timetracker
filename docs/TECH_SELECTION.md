# Windows時間追跡アプリ 技術選定

## 要件

- アプリケーションの作業時間を自動記録（ManicTime風）
- 軽量に動作
- 別途ランタイムインストール不要
- Windowsネイティブアプリ

## 必要な機能

1. **アクティブウィンドウの監視** - `GetForegroundWindow()` API
2. **プロセス情報取得** - アプリ名、ウィンドウタイトル
3. **バックグラウンド実行** - システムトレイ常駐
4. **データ永続化** - SQLiteなど
5. **UI** - タイムライン表示、統計・レポート

---

## 技術オプション比較

| 技術 | メモリ使用量 | バイナリサイズ | ランタイム | 開発効率 | Windows API |
|------|------------|--------------|----------|---------|-------------|
| **Tauri 2.0** | 30-50 MB | 2.5-10 MB | WebView2 (Win10/11標準) | ★★★★★ | Rust経由で完全対応 |
| **Rust + egui** | 10-30 MB | 2-5 MB | 不要 | ★★★☆☆ | windows-rs で完全対応 |
| **Rust + iced** | 15-40 MB | 3-8 MB | 不要 | ★★★☆☆ | windows-rs で完全対応 |
| **C++ Win32** | 5-15 MB | 0.5-2 MB | 不要 | ★★☆☆☆ | ネイティブ |
| **C# WinForms (Self-Contained)** | 50-100 MB | 60-150 MB | 同梱 | ★★★★☆ | P/Invoke対応 |
| **Electron** | 150-300 MB | 80-150 MB | 同梱 | ★★★★★ | 制限あり |

---

## 推奨: Tauri 2.0

### 選定理由

1. **ランタイム不要（実質）**
   - WebView2はWindows 10/11に標準搭載
   - Windows 7/8.1はサポート終了済み（2023年1月）

2. **軽量**
   - メモリ: 30-50 MB（Electronの1/5以下）
   - バイナリ: 2.5-10 MB（Electronの1/30以下）

3. **開発効率**
   - UIはWeb技術（React/Vue/Svelte/Vanilla）
   - バックエンドはRust（高速・安全）
   - 豊富なプラグインエコシステム

4. **Windows API完全対応**
   - Rustからwindows-rsクレートで全APIアクセス可能
   - `GetForegroundWindow()`, `GetWindowText()` 等

5. **Tauri 2.0の新機能（2024年10月リリース）**
   - システムトレイ改善
   - 自動アップデート機能
   - より軽量なコア

### 参考: 類似アプリの技術スタック

- **[ActivityWatch](https://activitywatch.net/)**: Python → Rust移行中
- **ManicTime**: .NET WPF
- **RescueTime**: Electron

---

## 次点: Rust + egui

最も軽量を追求する場合の選択肢。

### メリット
- 完全にランタイム不要
- メモリ使用量が最小（10-30 MB）
- 単一実行ファイル

### デメリット
- UI開発の学習コスト
- Web技術と比べてUI作成に時間がかかる
- エコシステムがまだ成熟途上

---

## 非推奨オプション

### C# .NET (WinForms/WPF)

- **Native AOT非対応**: WinForms/WPFは.NET 8でもNative AOT未対応
- **Self-Contained**: ランタイム同梱で60-150 MBになる
- 軽量要件を満たさない

### Electron

- メモリ150-300 MB、バイナリ80-150 MB
- 「軽量」の要件に完全に反する

### C++ Win32

- 最も軽量だが開発コストが非常に高い
- UI作成が困難
- 現代的な開発には向かない

---

## 結論

| 優先度 | 技術 | 推奨理由 |
|--------|------|----------|
| **1位** | Tauri 2.0 | バランス最良。軽量・高開発効率・ランタイム実質不要 |
| **2位** | Rust + egui | 最軽量。学習コスト許容なら最適 |
| **3位** | Rust + iced | eguiより洗練されたUI、Elm風アーキテクチャ |

## 推奨アーキテクチャ（Tauri 2.0の場合）

```
┌─────────────────────────────────────────────┐
│                  Frontend                    │
│         (React/Vue/Svelte + TypeScript)      │
│  ┌─────────────┐ ┌─────────────────────────┐ │
│  │ Timeline UI │ │ Statistics Dashboard    │ │
│  └─────────────┘ └─────────────────────────┘ │
└─────────────────────────────────────────────┘
                      │ Tauri IPC
┌─────────────────────────────────────────────┐
│                  Backend (Rust)              │
│  ┌─────────────┐ ┌──────────────┐           │
│  │ Window      │ │ Data Storage │           │
│  │ Watcher     │ │ (SQLite)     │           │
│  │ (windows-rs)│ └──────────────┘           │
│  └─────────────┘                            │
└─────────────────────────────────────────────┘
```

## 次のステップ

1. Tauri 2.0プロジェクトの初期化
2. ウィンドウ監視モジュールの実装（Rust + windows-rs）
3. SQLiteによるデータ永続化
4. フロントエンドUI実装
5. システムトレイ統合

---

## 参考リンク

- [Tauri 2.0](https://v2.tauri.app/)
- [ActivityWatch](https://activitywatch.net/)
- [windows-rs](https://github.com/microsoft/windows-rs)
- [egui](https://github.com/emilk/egui)
- [iced](https://github.com/iced-rs/iced)
