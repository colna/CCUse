import type { ComponentPropsWithoutRef } from "react";
import type { MDXComponents } from "mdx/types";

type ElementProps<TagName extends keyof JSX.IntrinsicElements> =
  ComponentPropsWithoutRef<TagName>;

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    h1: ({ children, ...props }: ElementProps<"h1">) => (
      <h1
        className="font-display text-4xl font-semibold leading-apple-headline"
        {...props}
      >
        {children}
      </h1>
    ),
    h2: ({ children, ...props }: ElementProps<"h2">) => (
      <h2
        className="mt-10 scroll-mt-28 font-display text-2xl font-semibold leading-apple-tile"
        {...props}
      >
        {children}
      </h2>
    ),
    h3: ({ children, ...props }: ElementProps<"h3">) => (
      <h3
        className="mt-8 scroll-mt-28 text-xl font-semibold leading-7"
        {...props}
      >
        {children}
      </h3>
    ),
    p: (props: ElementProps<"p">) => (
      <p className="mt-4 leading-7 text-muted-foreground" {...props} />
    ),
    a: ({ children, ...props }: ElementProps<"a">) => (
      <a
        className="font-medium text-primary underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
        {...props}
      >
        {children}
      </a>
    ),
    ul: (props: ElementProps<"ul">) => (
      <ul
        className="mt-4 grid gap-2 pl-5 leading-7 text-muted-foreground marker:text-primary"
        {...props}
      />
    ),
    ol: (props: ElementProps<"ol">) => (
      <ol
        className="mt-4 grid gap-2 pl-5 leading-7 text-muted-foreground marker:text-primary"
        {...props}
      />
    ),
    pre: (props: ElementProps<"pre">) => (
      <pre
        className="mt-5 overflow-x-auto rounded-lg border border-border bg-zinc-950 p-4 text-sm text-zinc-50"
        {...props}
      />
    ),
    code: (props: ElementProps<"code">) => (
      <code
        className="rounded-md bg-muted px-1.5 py-0.5 font-mono text-sm text-foreground"
        {...props}
      />
    ),
    ...components,
  };
}
