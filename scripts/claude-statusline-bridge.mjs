#!/usr/bin/env node
// Claude Code 상태줄 연결: Claude Code가 넘겨주는 JSON에서 한도 값만 꺼내
// ~/.deskdusk/claude-limits.json 에 적고, 상태줄에는 한 줄 요약을 보여준다.
// 네트워크 호출 없음. 인증 정보는 읽지 않는다.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const dir = path.join(os.homedir(), ".deskdusk");
const target = path.join(dir, "claude-limits.json");

const toSeconds = (v) => {
  if (typeof v === "number") return v > 1e12 ? Math.round(v / 1000) : Math.round(v);
  if (typeof v === "string") {
    const t = Date.parse(v);
    return Number.isNaN(t) ? 0 : Math.round(t / 1000);
  }
  return 0;
};
const pick = (w) => {
  if (!w || typeof w !== "object") return null;
  const used = Number(w.used_percentage ?? w.used_percent);
  if (!Number.isFinite(used)) return null;
  return { used_percentage: used, resets_at: toSeconds(w.resets_at ?? w.reset_at) };
};

let text = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (c) => { if (text.length < 512 * 1024) text += c; });
process.stdin.on("end", () => {
  let out = "";
  try {
    const payload = JSON.parse(text);
    const rl = payload.rate_limits || {};
    const five = pick(rl.five_hour), seven = pick(rl.seven_day);
    if (five || seven) {
      const value = { observed_at: Math.round(Date.now() / 1000), five_hour: five, seven_day: seven };
      try {
        fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
        const temp = `${target}.${process.pid}.tmp`;
        fs.writeFileSync(temp, JSON.stringify(value), { mode: 0o600 });
        fs.renameSync(temp, target);
      } catch {} // 디스크가 가득 차도 Claude Code를 방해하지 않는다.
      const parts = [];
      if (five) parts.push(`5시간 ${Math.round(five.used_percentage)}%`);
      if (seven) parts.push(`주간 ${Math.round(seven.used_percentage)}%`);
      out = parts.join(" · ");
    }
    const model = payload.model && (payload.model.display_name || payload.model.id);
    if (model) out = out ? `${model} · ${out}` : String(model);
  } catch {}
  process.stdout.write(out);
});
