# 設計資料

2026-11-19 時点の写し。claude.ai 上で作った要件定義・設計書と画面モックを、実装用にまとめたもの。

## 中身

| ファイル | 役割 |
| --- | --- |
| `requirements-design.md` | 要件定義・設計書の全文。機能要件41件、非機能要件、リリース段階、アーキテクチャ、DDL、主要ロジック、画面構成、コマンド一覧、未決事項。v1.1の詳細は、この写しに足したもの(原本にはない) |
| `implementation-plan.md` | MVPまでのフェーズ分けと完了条件 |
| `implementation-plan-v1.1.md` | v1.1のフェーズ分け(フェーズ10〜16)と完了条件、進め方。このリポジトリで作ったもの(原本にはない) |
| `diagrams/architecture.png` | アプリの構成図(設計書の図) |
| `diagrams/status-transitions.png` | ステータスの遷移図(設計書の図) |
| `diagrams/screens.png` | 画面のつながり(設計書の図) |
| `ui-mock/` | 6画面のモック。`README.md` に読み方、`screenshots/` に画面キャプチャ、`.dc.html` にソース |
| `aizome/README.md` | デザインシステム Aizome の方針(色、文字、余白、文言) |
| `aizome/tokens.css` | Aizome のトークン(CSS変数)。ライトとダーク |
| `aizome/components/bundle.css` | Aizome の14部品のスタイル |
| `aizome/components/bundle.js` | Aizome の14部品の実装(グローバルの React を前提にした配布形式)。移植の参考 |
| `aizome/components/index.d.ts` | 14部品の props の型定義 |

## 食い違ったときの優先順位

1. 動作とデータは `requirements-design.md`
2. 見た目、配置、文言は `ui-mock/`
3. 部品とトークンの使い方は `aizome/`

設計書とモックで動作が食い違う箇所を見つけたら、設計書に合わせたうえで報告する。

## 写しを使うときの注意

- **フォント**: `bundle.css` の1行目は Google Fonts を読み込む `@import`。このアプリは外部と通信しない要件なので、この行は外し、IBM Plex Sans JP(400 / 500 / 600 / 700)と IBM Plex Mono(400 / 500)をアプリに同梱する。
- **tokens.css**: Aizome の原本には `tokens.json` しかなく、この `tokens.css` はそこから手で書き起こしたもの。`bundle.css` が参照する変数はすべて定義済みであることを確認している。
- **bundle.js**: `window.React` を前提にした即時関数で、そのままでは Vite のモジュールとして読み込めない。TypeScript のコンポーネントに移植する。内蔵アイコン26種のパスは `ICONS` の表にある。
- **アイコンの不足**: モックは内蔵アイコンだけで描いている。足りない形は Aizome の方針どおり、同じ線幅(1.75)・丸い線端の線画で足す。
- **図**: 設計書の図3点は画像にしてある。内容は本文と表にも書いてある。
- **モック**: `.dc.html` は claude.ai のデザインキャンバス用の形式で、単体では表示できない。読み方は `ui-mock/README.md`。

## 原本

利用者本人だけが開ける claude.ai 上のページ。実装中に参照する必要はない。

- 要件定義・設計書: https://claude.ai/artifact/KKtxnzkSipfhw82ahTnVGD
- UIデザイン(6画面): https://claude.ai/artifact/UTzvV1PcoMBwKZVCncgSmF
- Aizome デザインシステム: https://claude.ai/artifact/TfnRLCnw9rWYijm2vxJkE6
