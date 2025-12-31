import React from 'react';

type AnimatedWordmarkProps = {
  text?: string;
  className?: string;
};

export default function AnimatedWordmark({ text = 'Emdash2', className }: AnimatedWordmarkProps) {
  return (
    <h1
      className={
        ['wordmark-shimmer select-none text-6xl font-bold text-foreground', className]
          .filter(Boolean)
          .join(' ')
      }
      style={{
        fontFamily: 'monospace',
        letterSpacing: '0.1em',
        textShadow: '2px 2px 0px rgba(0, 0, 0, 0.18)',
      }}
    >
      <span
        aria-hidden="true"
        className="mr-3 inline-block align-middle text-muted-foreground/70 wordmark-dash"
      >
        —
      </span>
      <span className="align-middle">{text}</span>
    </h1>
  );
}
