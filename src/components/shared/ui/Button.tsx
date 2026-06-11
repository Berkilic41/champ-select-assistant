import React from 'react';
import clsx from 'clsx';
import './ui.css';

type ButtonVariant = 'primary' | 'ghost' | 'subtle';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

/** Unified button — `primary` is the gold-gradient CTA, `ghost`/`subtle` for the rest. */
export const Button: React.FC<ButtonProps> = ({
  variant = 'subtle',
  className,
  children,
  type = 'button',
  ...rest
}) => (
  // eslint-disable-next-line react/button-has-type
  <button className={clsx('ui-button', `ui-button--${variant}`, className)} type={type} {...rest}>
    {children}
  </button>
);
