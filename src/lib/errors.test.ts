import { describe, expect, it } from 'vitest';
import { errCode, errText } from './errors';

describe('命令错误取值', () => {
  it('新命令返回的对象错误取 message 展示', () => {
    const e = { code: 'backup-version', message: '备份文件版本 3 高于当前支持的版本 2' };
    expect(errText(e)).toBe('备份文件版本 3 高于当前支持的版本 2');
    expect(errCode(e)).toBe('backup-version');
  });

  it('还没迁移的命令仍是纯字符串', () => {
    expect(errText('保存失败: 端口被占用')).toBe('保存失败: 端口被占用');
    expect(errCode('保存失败: 端口被占用')).toBeNull();
  });

  it('对象里缺字段时按未知错误处理，不把整个对象打印给用户', () => {
    expect(errText({ code: 'io' })).toBe('[object Object]');
    expect(errCode({ message: '缺 code' })).toBeNull();
  });

  it('异常对象走兜底字符串', () => {
    expect(errText(new Error('boom'))).toBe('Error: boom');
    expect(errCode(new Error('boom'))).toBeNull();
  });
});
