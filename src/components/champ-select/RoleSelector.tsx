import React from 'react';
import { useTranslation } from 'react-i18next';
import './RoleSelector.css';

/** LCU canonical position keys (lowercase) — the engine keys meta/role lookups on these. */
export const ROLES = ['top', 'jungle', 'middle', 'bottom', 'utility'] as const;
export type Role = (typeof ROLES)[number];

export type RoleSource = 'lcu' | 'manual' | 'preferred' | 'none';

interface Props {
  /** Currently effective role (one of ROLES, or '' when unknown). */
  role: string;
  /** Where the effective role came from. 'none' → prompt the user to choose. */
  source: RoleSource;
  onChange: (role: Role) => void;
}

/**
 * Compact role picker shown in champ-select. The whole coaching stack
 * (recommendations, bans, lane matchup, role-fit) keys off the local player's
 * assigned position; LCU leaves it empty in Blind/Normal/Quickplay, so this lets
 * the user set it. Auto-selects the LCU role when present.
 */
export const RoleSelector: React.FC<Props> = ({ role, source, onChange }) => {
  const { t } = useTranslation();
  const unknown = source === 'none' || role === '';

  return (
    <div className={`role-selector${unknown ? ' role-selector--prompt' : ''}`}>
      <span className="role-selector__label">
        {unknown ? t('champSelect.rolePrompt') : t('champSelect.roleTitle')}
      </span>
      <div className="role-selector__btns" role="group" aria-label={t('champSelect.roleTitle')}>
        {ROLES.map((r) => (
          <button
            key={r}
            type="button"
            className={`role-selector__btn${role === r ? ' role-selector__btn--active' : ''}`}
            aria-pressed={role === r}
            onClick={() => onChange(r)}
          >
            {t(`champSelect.roles.${r}`)}
          </button>
        ))}
      </div>
      {!unknown && (
        <span className="role-selector__hint">
          {source === 'lcu'
            ? t('champSelect.roleAutoHint')
            : source === 'preferred'
              ? t('champSelect.rolePreferredHint')
              : t('champSelect.roleManualHint')}
        </span>
      )}
    </div>
  );
};
