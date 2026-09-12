/**
 * i18n 入口模块
 *
 * - 集中管理 locale 状态（模块级单例 ref）
 * - 提供 t(key, vars?) 函数，支持 {var} 插值
 * - 类型推断：以 zh-CN 为基准，所有 locale 必须满足其 key 结构
 *
 * 用法：
 *   import { useI18n } from "@/i18n";
 *   const { t, locale, setLocale, locales } = useI18n();
 *   <h1>{{ t("nav.posts") }}</h1>
 *   {{ t("tags.counter", { tagCount: 5, postCount: 12 }) }}
 */
import { ref, computed, type Ref, type ComputedRef } from "vue";
import zhCN from "./locales/zh-CN";
import zhTW from "./locales/zh-TW";
import en from "./locales/en";

export type Locale = "zh-CN" | "zh-TW" | "en";

/** 以 zh-CN 为基准的类型（所有翻译必须满足此结构） */
type Messages = typeof zhCN;

const messages: Record<Locale, Messages> = {
  "zh-CN": zhCN as unknown as Messages,
  "zh-TW": zhTW as unknown as Messages,
  en: en as unknown as Messages,
};

export const LOCALES: { value: Locale; label: string; native: string }[] = [
  { value: "zh-CN", label: "Simplified Chinese", native: "简体中文" },
  { value: "zh-TW", label: "Traditional Chinese", native: "繁體中文" },
  { value: "en", label: "English", native: "English" },
];

const STORAGE_KEY = "x-site:locale";

/** 从 navigator.language 探测最接近的 locale */
function detectSystemLocale(): Locale {
  const lang = navigator.language || "en";
  const lower = lang.toLowerCase();
  if (lower.startsWith("zh")) {
    if (
      lower.includes("tw") ||
      lower.includes("hk") ||
      lower.includes("mo") ||
      lower.includes("hant")
    ) {
      return "zh-TW";
    }
    return "zh-CN";
  }
  return "en";
}

function loadInitialLocale(): Locale {
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as Locale | null;
    if (saved && LOCALES.some((l) => l.value === saved)) {
      return saved;
    }
  } catch {
    /* localStorage 不可用时降级 */
  }
  return detectSystemLocale();
}

// 全局共享的 locale 状态（模块级单例，所有 useI18n 调用共用一份）
const locale: Ref<Locale> = ref(loadInitialLocale());

/** 切换语言并持久化 */
function setLocale(next: Locale): void {
  locale.value = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    /* 忽略写入失败 */
  }
  document.documentElement.lang = next;
}

// 初始化时同步 <html lang="...">
document.documentElement.lang = locale.value;

/**
 * 翻译函数
 * @param key 形如 "nav.posts" / "settings.preview.started"
 * @param vars 插值变量，会替换 {varName} 占位符
 * @returns 翻译后的字符串；找不到 key 时返回 key 本身（便于排查）
 */
function t(key: string, vars?: Record<string, string | number>): string {
  const parts = key.split(".");
  let cur: any = messages[locale.value];
  for (const p of parts) {
    if (cur && typeof cur === "object" && p in cur) {
      cur = cur[p];
    } else {
      return key;
    }
  }
  if (typeof cur !== "string") return key;
  if (vars) {
    return cur.replace(/\{(\w+)\}/g, (_, k: string) =>
      k in vars ? String(vars[k]) : `{${k}}`,
    );
  }
  return cur;
}

/** 当前语言的母语显示名（用于下拉框默认项） */
const currentNative: ComputedRef<string> = computed(
  () => LOCALES.find((l) => l.value === locale.value)?.native ?? "English",
);

export function useI18n() {
  return {
    locale,
    setLocale,
    locales: LOCALES,
    currentNative,
    t,
  };
}

// 兼容旧 import 路径
export { useI18n as useLocale };
