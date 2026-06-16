// dependency-cruiser config — 給新專案複製過去用
// 詳見 https://github.com/sverweij/dependency-cruiser
// 來源：~/.claude/templates/.dependency-cruiser.cjs
//
// 使用：
//   cp ~/.claude/templates/.dependency-cruiser.cjs <repo>/.dependency-cruiser.cjs
//   lefthook 已配在 pre-commit 自動跑
//
// 手動跑：depcruise src --config .dependency-cruiser.cjs

/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    // ===== 循環依賴必擋（決策 11）=====
    {
      name: "no-circular",
      severity: "error",
      comment: "循環依賴會導致初始化順序不可預測、難 tree-shake",
      from: {},
      to: { circular: true },
    },

    // ===== 不允許 orphans（dead code）=====
    {
      name: "no-orphans",
      severity: "warn",
      comment: "孤立模組（無人 import）可能是死 code",
      from: {
        orphan: true,
        pathNot: [
          "(^|/)\\.[^/]+\\.(js|cjs|mjs|ts|tsx)$", // dotfiles
          "\\.d\\.ts$",                            // type-only
          "(^|/)tsconfig\\.json$",
          "(^|/)(babel|webpack)\\.config\\.(js|cjs|mjs|ts)$",
        ],
      },
      to: {},
    },

    // ===== dev dep 不能被 prod code import =====
    {
      name: "no-deps-on-dev-deps",
      severity: "error",
      comment: "Prod code 不該 import devDependencies",
      from: { pathNot: "^(test|spec|tests|specs)/" },
      to: { dependencyTypes: ["npm-dev"] },
    },

    // ===== 不允許 import unresolvable =====
    {
      name: "not-to-unresolvable",
      severity: "error",
      from: {},
      to: { couldNotResolve: true },
    },

    // ===== 不允許 import 已 deprecated 套件 =====
    {
      name: "not-to-deprecated",
      severity: "warn",
      from: {},
      to: { dependencyTypes: ["deprecated"] },
    },

    // ===== Layered architecture：services 不該 import components =====
    {
      name: "service-not-import-ui",
      severity: "error",
      comment: "Services / API 層不該依賴 UI 層（決策 9 SSOT、層次分離）",
      from: { path: "^src/(services|api|core)/" },
      to: { path: "^src/(components|pages|features)/" },
    },
  ],

  options: {
    // TS 設定
    tsConfig: { fileName: "tsconfig.json" },
    tsPreCompilationDeps: true,

    // 不掃這些目錄
    doNotFollow: {
      path: "node_modules|dist|build|\\.next",
    },

    // 排除
    exclude: {
      path: "(^|/)(node_modules|dist|build|coverage|\\.next|\\.turbo)/",
    },

    // 報告
    reporterOptions: {
      dot: { theme: { graph: { rankdir: "TD" } } },
    },
  },
};
