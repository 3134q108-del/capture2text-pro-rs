export default function SpeechTab() {
  return (
    <div style={{ padding: "24px", color: "var(--text-secondary)" }}>
      <h3>語音朗讀設定</h3>
      <p>
        Azure TTS 設定尚未完成。T52 第一階段已移除本機 Qwen3-TTS，下一階段會加入 API key、
        region、各語言 voice 選擇與預覽。
      </p>
    </div>
  );
}
