// 命令错误的统一取值方式。
// 后端迁移中的命令返回 { code, message }，还没迁的仍返回纯字符串。
// 展示一律拿 message，判定一律拿 code —— 别在组件里各写一份形状判断。
export interface AppError {
  code: string;
  message: string;
}

function asAppError(e: unknown): AppError | null {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
    const { code, message } = e as Record<string, unknown>;
    if (typeof code === 'string' && typeof message === 'string') return { code, message };
  }
  return null;
}

/** 给用户看的错误文本。 */
export function errText(e: unknown): string {
  if (typeof e === 'string') return e;
  const err = asAppError(e);
  if (err) return err.message;
  return String(e);
}

/** 给程序判断的错误码；未迁移的命令返回 null。 */
export function errCode(e: unknown): string | null {
  return asAppError(e)?.code ?? null;
}
