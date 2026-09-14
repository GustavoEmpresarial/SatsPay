import { useState } from 'react';
import { useTranslation } from 'react-i18next';

type Props = Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type'> & {
  label?: string;
};

export function PasswordInput({ label, className, id, ...rest }: Props) {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);
  const inputId = id ?? 'password';

  return (
    <div className="w-full">
      {label && (
        <label className="label" htmlFor={inputId}>
          {label}
        </label>
      )}
      <div className="relative w-full">
        <input
          {...rest}
          id={inputId}
          type={visible ? 'text' : 'password'}
          className={`input w-full pr-11 ${className ?? ''}`}
          autoComplete={rest.autoComplete ?? 'current-password'}
        />
        <button
          type="button"
          tabIndex={-1}
          aria-label={visible ? t('common.hidePassword') : t('common.showPassword')}
          onClick={() => setVisible((v) => !v)}
          className="absolute inset-y-0 right-0 flex w-11 items-center justify-center text-ink-muted transition hover:text-ink"
        >
          <i className={`bi ${visible ? 'bi-eye-slash' : 'bi-eye'}`} aria-hidden />
        </button>
      </div>
    </div>
  );
}
