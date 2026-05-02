import { Button, Section, SectionBody, SectionHeader } from "@/components/ui";

export default function HelpTab() {
  return (
    <div className="flex flex-col gap-4">
      <Section>
        <SectionHeader title="使用說明" />
      </Section>

      <Section>
        <SectionHeader title="程式用途" />
        <SectionBody>
          <p className="text-sm text-muted-foreground">
            Capture2Text Pro 是駐留系統匣的桌面 OCR + 翻譯工具。截圖辨識 + 智慧雙向翻譯 + Azure TTS
            朗讀，支援 32 種語言（主推 5 / 常用 7 / 進階 8 / 實驗 12），翻譯全本機處理（不走雲端，僅 TTS 可選 Azure）。
          </p>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="使用流程" />
        <SectionBody>
          <ol className="list-decimal space-y-1 pl-5 text-sm text-muted-foreground">
            <li>按下任一快捷鍵（Win+Q / Win+W / Win+E）觸發截圖</li>
            <li>結果視窗自動彈出顯示原文 + 翻譯</li>
            <li>可點 Speak 聆聽，Copy 複製到剪貼簿</li>
          </ol>
          <div className="mt-3 space-y-1 text-sm text-muted-foreground">
            <p>需要做的設定（可選）：</p>
            <p>A. 啟用語言：在「語言」tab 勾選要使用的語言（預設啟用主推 5 語）</p>
            <p>B. 母語 + 目標語言：在「翻譯」tab 設定（預設母語 zh-TW、目標 en-US；智慧對翻會自動切方向，抓母語翻成目標、抓其他語言翻成母語）</p>
            <p>C. Azure TTS（用於 Speak 朗讀）：F0 Free tier 月 500K 字符免費，設定步驟見下</p>
          </div>
        </SectionBody>
      </Section>

      <Section>
        <SectionHeader title="Azure 金鑰申請（7 步）" />
        <SectionBody>
          <ol className="list-decimal space-y-2 pl-5 text-sm text-muted-foreground">
            <li>
              前往 Azure Portal：
              <Button asChild type="button" variant="ghost" size="sm" className="ml-1 px-1">
                <a href="https://portal.azure.com" target="_blank" rel="noreferrer">
                  portal.azure.com
                </a>
              </Button>
              註冊帳號（F0 免費不需信用卡）
            </li>
            <li>左側選單點「+ 建立資源」→ 搜尋「Speech service」→ 點擊建立</li>
            <li>
              填寫：
              <ul className="list-disc space-y-1 pl-5">
                <li>訂用帳戶：Free Trial 或自有</li>
                <li>資源群組：新建一個（例：capture2text-rg）</li>
                <li>區域：East Asia（台灣優先）/ Southeast Asia / Japan East 三選一</li>
                <li>名稱：任取（全 Azure 唯一）</li>
                <li>定價層：F0 Free</li>
              </ul>
            </li>
            <li>點「檢閱 + 建立」→「建立」，等候約 30 秒</li>
            <li>部署完成後，點「前往資源」</li>
            <li>左側選單「金鑰與端點」→ 複製「金鑰 1」（32 字元 hex）</li>
            <li>貼到本程式「設定 → 語音 → 訂閱金鑰」，Region 選同個，點「儲存並測試」</li>
          </ol>
          <p className="mt-3 text-sm text-muted-foreground">
            注意：配額用完（月 500K）會失敗，下個月 1 號自動 reset。S0 Standard 是付費方案（約
            $1/百萬字符），設定中可切換。
          </p>
        </SectionBody>
      </Section>
    </div>
  );
}
