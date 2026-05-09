import { Button as AntButton, type ButtonProps as AntButtonProps } from "antd";
import { forwardRef } from "react";

type AntType = AntButtonProps["type"];
type AntSize = AntButtonProps["size"];
type AntVariant = AntButtonProps["variant"];

type LegacyVariant =
  | "default"
  | "destructive"
  | "outline"
  | "secondary"
  | "ghost"
  | "link"
  | "pill";
type LegacySize = "default" | "sm" | "lg" | "icon";

export interface ButtonProps extends Omit<
  AntButtonProps,
  "type" | "size" | "variant"
> {
  /** Accepts either antd type values or legacy shadcn variant strings. */
  type?: AntType | LegacyVariant | "button" | "submit" | "reset";
  size?: AntSize | LegacySize;
  variant?: AntVariant | LegacyVariant;
}

const LEGACY_VARIANT_TO_ANT: Record<
  LegacyVariant,
  { type: AntType; danger?: boolean; variant?: AntVariant }
> = {
  default: { type: "primary" },
  destructive: { type: "primary", danger: true },
  outline: { type: "default" },
  secondary: { type: "default" },
  ghost: { type: "text" },
  link: { type: "link" },
  pill: { type: "default" },
};

const LEGACY_SIZE_TO_ANT: Record<LegacySize, AntSize> = {
  default: "middle",
  sm: "small",
  lg: "large",
  icon: "small",
};

function isLegacyVariant(value: unknown): value is LegacyVariant {
  return (
    typeof value === "string" &&
    [
      "default",
      "destructive",
      "outline",
      "secondary",
      "ghost",
      "link",
      "pill",
    ].includes(value)
  );
}

function isLegacySize(value: unknown): value is LegacySize {
  return (
    typeof value === "string" && ["default", "sm", "lg", "icon"].includes(value)
  );
}

export const Button = forwardRef<HTMLElement, ButtonProps>(
  (
    { autoInsertSpace = false, type, size, variant, danger, htmlType, ...rest },
    ref,
  ) => {
    let resolvedType: AntType = "default";
    let resolvedDanger = danger;
    let resolvedVariant: AntVariant | undefined;
    let resolvedHtmlType = htmlType;

    if (type === "button" || type === "submit" || type === "reset") {
      resolvedHtmlType = type;
    } else if (isLegacyVariant(type)) {
      const mapped = LEGACY_VARIANT_TO_ANT[type];
      resolvedType = mapped.type;
      if (mapped.danger) resolvedDanger = mapped.danger;
    } else if (type) {
      resolvedType = type as AntType;
    }

    if (variant) {
      if (isLegacyVariant(variant)) {
        const mapped = LEGACY_VARIANT_TO_ANT[variant];
        resolvedType = mapped.type ?? resolvedType;
        if (mapped.danger != null) resolvedDanger = mapped.danger;
      } else {
        resolvedVariant = variant;
      }
    }

    const resolvedSize: AntSize = isLegacySize(size)
      ? LEGACY_SIZE_TO_ANT[size]
      : (size as AntSize);

    return (
      <AntButton
        ref={ref as never}
        autoInsertSpace={autoInsertSpace}
        type={resolvedType}
        size={resolvedSize}
        danger={resolvedDanger}
        variant={resolvedVariant}
        htmlType={resolvedHtmlType}
        {...rest}
      />
    );
  },
);
Button.displayName = "Button";
