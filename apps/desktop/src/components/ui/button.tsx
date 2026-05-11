import { Button as AntButton, type ButtonProps as AntButtonProps } from "antd";
import { forwardRef } from "react";

/**
 * 项目统一的 `Button`：仅是 antd `Button` 的薄壳，把
 * `autoInsertSpace` 默认设为 `false` —— antd 6 默认会在两个汉字之间插入
 * 空格，这与产品视觉规范（按钮内中文连写）冲突，所以全局关掉。
 *
 * 其余 prop 一律透传 antd，保留 `forwardRef` 兼容 antd 的 ref 类型
 * （antd 内部用 `HTMLElement` 兜全部按钮形态）。
 */
export type ButtonProps = AntButtonProps;

export const Button = forwardRef<HTMLElement, ButtonProps>(
  ({ autoInsertSpace = false, ...rest }, ref) => (
    <AntButton ref={ref as never} autoInsertSpace={autoInsertSpace} {...rest} />
  ),
);
Button.displayName = "Button";
