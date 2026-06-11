import React from 'react';
import { useTranslation } from 'react-i18next';
import { Layers } from 'lucide-react';
import {
  Recommendation,
  CounterPickHint,
  TeamCompBoard as TeamCompBoardData,
  ComboBoardEntry,
  CounterItemHint,
  LaneMatchup,
  GamePlan,
} from '../../../types/recommendation';
import type { DraftSimResult } from '../../../types/generated/DraftSimResult';
import type { DraftFork } from '../../../types/generated/DraftFork';
import { Tabs, EmptyState, type TabItem } from '../../shared/ui';
import { LaneMatchupPanel } from '../LaneMatchupPanel';
import { CounterItemsPanel } from '../CounterItemsPanel';
import { GamePlanPanel } from '../GamePlanPanel';
import { TeamCompBoard } from '../TeamCompBoard';
import { CounterPickBoard } from '../CounterPickBoard';
import { ComboBoard } from '../ComboBoard';
import { DeepDiveTab } from './DeepDiveTab';
import './SecondaryTabs.css';

export type SecondaryTab = 'matchup' | 'teamplan' | 'counters' | 'deepdive';

interface Props {
  rec?: Recommendation | null;
  laneMatchup?: LaneMatchup | null;
  counterItems?: CounterItemHint[];
  gamePlan?: GamePlan | null;
  teamComp?: TeamCompBoardData | null;
  counterPicks?: CounterPickHint[];
  comboBoard?: ComboBoardEntry[];
  draftSimulation?: DraftSimResult[] | null;
  draftFork?: DraftFork | null;
  activeTab: SecondaryTab;
  onTabChange: (tab: SecondaryTab) => void;
}

/** The secondary depth, organised into tabs (the only scroll region of the dashboard).
 *  Build lives in the decision column; these tabs carry matchup / team plan / counters /
 *  the deep-dive. Each tab degrades to an honest empty state. */
export const SecondaryTabs: React.FC<Props> = ({
  rec,
  laneMatchup,
  counterItems = [],
  gamePlan,
  teamComp,
  counterPicks = [],
  comboBoard = [],
  draftSimulation,
  draftFork,
  activeTab,
  onTabChange,
}) => {
  const { t } = useTranslation();
  const tabs: TabItem[] = [
    { value: 'matchup', label: t('secondaryTabs.matchup', { defaultValue: 'Matchup' }) },
    { value: 'teamplan', label: t('secondaryTabs.teamPlan', { defaultValue: 'Takım planı' }) },
    { value: 'counters', label: t('secondaryTabs.counters', { defaultValue: 'Counter / Combo' }) },
    { value: 'deepdive', label: t('secondaryTabs.deepDive', { defaultValue: 'Detay' }) },
  ];
  const emptyMsg = t('secondaryTabs.empty', {
    defaultValue: 'Henüz veri yok — draft ilerledikçe dolacak.',
  });
  const empty = <EmptyState icon={<Layers size={18} />} message={emptyMsg} />;

  return (
    <div className="secondary-tabs">
      <Tabs
        tabs={tabs}
        value={activeTab}
        onChange={(v) => onTabChange(v as SecondaryTab)}
        ariaLabel={t('secondaryTabs.aria', { defaultValue: 'Detay sekmeleri' })}
        layoutId="secondary-tab-underline"
      />
      <div className="secondary-tabs__panel" role="tabpanel">
        {activeTab === 'matchup' &&
          (laneMatchup || counterItems.length > 0 ? (
            <>
              <LaneMatchupPanel matchup={laneMatchup} />
              <CounterItemsPanel items={counterItems} />
            </>
          ) : (
            empty
          ))}

        {activeTab === 'teamplan' &&
          (gamePlan || teamComp ? (
            <>
              <GamePlanPanel plan={gamePlan} />
              <TeamCompBoard comp={teamComp} />
            </>
          ) : (
            empty
          ))}

        {activeTab === 'counters' &&
          (counterPicks.length > 0 || comboBoard.length > 0 ? (
            <>
              <CounterPickBoard picks={counterPicks} />
              <ComboBoard combos={comboBoard} />
            </>
          ) : (
            empty
          ))}

        {activeTab === 'deepdive' &&
          (rec ? (
            <DeepDiveTab rec={rec} draftSimulation={draftSimulation} draftFork={draftFork} />
          ) : (
            empty
          ))}
      </div>
    </div>
  );
};
