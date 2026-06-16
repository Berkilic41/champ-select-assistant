import React, { useEffect, useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Compass } from 'lucide-react';
import { ChampSelectSession, Recommendation, BanSuggestion, GamePlan, CounterPickHint, TeamCompBoard as TeamCompBoardData, ComboBoardEntry, DraftVerdict, CounterItemHint, LaneMatchup } from '../../types/recommendation';
import type { DataSourceRegistryReport } from '../../types/generated/DataSourceRegistryReport';
import type { DraftBrainQualityReport } from '../../types/generated/DraftBrainQualityReport';
import type { FeedbackAnalytics } from '../../types/generated/FeedbackAnalytics';
import type { FeedbackObservabilityReport } from '../../types/generated/FeedbackObservabilityReport';
import type { PerformanceReport } from '../../types/generated/PerformanceReport';
import type { DraftSimResult } from '../../types/generated/DraftSimResult';
import type { DraftFork } from '../../types/generated/DraftFork';
import type { PipelineQualityReport } from '../../types/generated/PipelineQualityReport';
import type { DataTrajectoryView } from '../../types/generated/DataTrajectoryView';
import { PhaseView } from '../../hooks/useChampSelectPhase';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { PhaseHeader } from './PhaseHeader';
import { RoleSelector, type Role, type RoleSource } from './RoleSelector';
import { DataStatusBadges } from './DataStatusBadges';
import { DraftFlow } from './DraftFlow';
import { HeroCard } from './HeroCard';
import { PostLockFeedback } from './PostLockFeedback';
import { QuickPickList } from './QuickPickList';
import { AlternativesRail } from './dashboard/AlternativesRail';
import { SecondaryTabs, type SecondaryTab } from './dashboard/SecondaryTabs';
import { TeamSlotView } from './TeamSlotView';
import { BuildSummary } from './BuildSummary';
import { GamePlanPanel } from './GamePlanPanel';
import { CounterPickBoard } from './CounterPickBoard';
import { TeamCompBoard } from './TeamCompBoard';
import { ComboBoard } from './ComboBoard';
import { CounterItemsPanel } from './CounterItemsPanel';
import { LaneMatchupPanel } from './LaneMatchupPanel';
import { DraftVerdictPanel } from './DraftVerdictPanel';
import { MetaTrendChip } from './MetaTrendChip';
import { PerformancePanel } from './PerformancePanel';
import { FeedbackAnalyticsPanel } from './FeedbackAnalyticsPanel';
import { DraftSimulatorPanel } from './DraftSimulatorPanel';
import { DraftForkPanel } from './DraftForkPanel';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
import { ChampionIcon } from '../shared/ChampionIcon';
import { BanSuggestionList } from './BanSuggestionList';
import { BanIcon } from './BanIcon';
import { assessPickClarity } from '../../lib/pickClarity';
import { Badge } from '../shared/ui';
import './ChampSelectScreen.css';

interface Props {
  session: ChampSelectSession;
  /** Effective lane role driving the coaching ('' = unknown). */
  role?: string;
  /** Where the role came from — 'none' surfaces the pick-your-role prompt. */
  roleSource?: RoleSource;
  /** Called when the user picks a role manually. */
  onRoleChange?: (role: Role) => void;
  recommendations: Recommendation[];
  /** Full analysis of the local player's LOCKED champion (build + game plan).
   *  When present, the finalization / post-lock views pin to THIS pick instead
   *  of recommendations[0] (which is a different champion once yours is locked). */
  lockedRec?: Recommendation | null;
  /** Team-level macro game plan for the current draft. */
  gamePlan?: GamePlan | null;
  /** Counters from the player's pool vs the visible lane opponent. */
  counterPicks?: CounterPickHint[];
  /** Both teams' composition summaries for the draft board. */
  teamComp?: TeamCompBoardData | null;
  /** Ally combos for the local player's pick. */
  comboBoard?: ComboBoardEntry[];
  /** Single decisive draft verdict. */
  draftVerdict?: DraftVerdict | null;
  /** Defensive counter-itemization vs enemy comp. */
  counterItems?: CounterItemHint[];
  /** Lane matchup read for the local pick. */
  laneMatchup?: LaneMatchup | null;
  /** Local/cloud data-source coverage report. */
  dataRegistryReport?: DataSourceRegistryReport | null;
  /** Production data-pipeline coverage/freshness/action report. */
  pipelineQualityReport?: PipelineQualityReport | null;
  /** Fused data trajectory (quality + background ramp motion). */
  dataTrajectory?: DataTrajectoryView | null;
  /** DraftBrain pack/sync freshness and confidence report. */
  draftBrainQualityReport?: DraftBrainQualityReport | null;
  /** Local feedback-loop counters + canonical personalization status. */
  feedbackObservabilityReport?: FeedbackObservabilityReport | null;
  /** Read-only feedback sentiment analytics for recommendation quality. */
  feedbackAnalytics?: FeedbackAnalytics | null;
  /** Local draft move simulation for the current recommendation candidates. */
  draftSimulation?: DraftSimResult[] | null;
  /** Recent-match post-game coach report. */
  performanceReport?: PerformanceReport | null;
  champMap: Map<number, string>;
  loading: boolean;
  phaseView: PhaseView;
  recError?: string | null;
  banSuggestions: BanSuggestion[];
  onFeedbackSubmitted?: () => void;
  onHoverChampion?: (championId: number) => void;
}

export function selectDraftForkIds(
  recommendations: Pick<Recommendation, 'champion_id'>[],
  activeIdx: number,
): [number, number] | null {
  const active = recommendations[activeIdx];
  const alternative = recommendations.find((rec) => rec.champion_id !== active?.champion_id);
  if (!active || !alternative) return null;
  return [active.champion_id, alternative.champion_id];
}

export const ChampSelectScreen: React.FC<Props> = ({
  session, role = '', roleSource = 'none', onRoleChange, recommendations, lockedRec, gamePlan, counterPicks = [], teamComp, comboBoard = [], draftVerdict, counterItems = [], laneMatchup, dataRegistryReport, pipelineQualityReport, dataTrajectory, draftBrainQualityReport, feedbackObservabilityReport, feedbackAnalytics, draftSimulation, performanceReport, champMap, loading, phaseView, recError, banSuggestions,
  onFeedbackSubmitted,
  onHoverChampion,
}) => {
  const { t } = useTranslation();
  const [activeIdx, setActiveIdx] = useState(0);
  const [secondaryTab, setSecondaryTab] = useState<SecondaryTab>('matchup');
  const [draftFork, setDraftFork] = useState<DraftFork | null>(null);
  const isActing = phaseView === 'pick_acting' || phaseView === 'ban_acting';
  const activeRec = recommendations[activeIdx];
  const forkIds = selectDraftForkIds(recommendations, activeIdx);
  const pickClarity = assessPickClarity(recommendations);

  useEffect(() => {
    if (activeIdx > 0 && activeIdx >= recommendations.length) {
      setActiveIdx(0);
    }
  }, [activeIdx, recommendations.length]);

  useEffect(() => {
    if (!forkIds) {
      setDraftFork(null);
      return;
    }
    const [optionAId, optionBId] = forkIds;
    let cancelled = false;
    invoke<DraftFork | null>('get_draft_fork', {
      sessionJson: session,
      optionAId,
      optionBId,
    })
      .then((fork) => {
        if (!cancelled) setDraftFork(fork);
      })
      .catch(() => {
        if (!cancelled) setDraftFork(null);
      });

    return () => {
      cancelled = true;
    };
  }, [session, forkIds?.[0], forkIds?.[1]]);

  useKeyboardShortcuts(
    recommendations,
    isActing,
    onHoverChampion ?? (() => {}),
    activeIdx,
    // During the pick phase, 1-5 switch the active card; in ban it keeps applying a hover.
    phaseView === 'pick_acting' ? setActiveIdx : undefined,
  );

  // Enemy team's current hover targets — shown during ban AND pick phases
  // (hover intent is visible in the LoL client UI anyway; no hidden info).
  const enemyThreats = session.their_team
    .filter(s => s.intent_champion_id > 0)
    .map(s => ({
      champId: s.intent_champion_id,
      champKey: champMap.get(s.intent_champion_id),
    }));

  // Locked-pick coaching: YOUR champion shown with the SAME decision card
  // (why + scores + game plan inline, "Detay" for the rest) plus its build.
  // Pins to the locked pick — never recommendations[0].
  const lockedView = lockedRec ? (
    <div className="cs-locked animate-fade-in">
      <DraftVerdictPanel verdict={draftVerdict} />
      <HeroCard rec={lockedRec} pinned />
      <PostLockFeedback rec={lockedRec} onFeedbackSubmitted={onFeedbackSubmitted} />
      {lockedRec.core_items.length > 0 && (
        <BuildSummary
          coreItems={lockedRec.core_items}
          situationalItems={lockedRec.situational_items}
          primaryRuneTree={lockedRec.primary_rune_tree}
          keystone={lockedRec.keystone}
          championName={lockedRec.champion_name || lockedRec.champion_key}
          skillOrder={lockedRec.skill_order}
          summonerSpells={lockedRec.summoner_spells}
          secondaryRunes={lockedRec.secondary_runes}
          statShards={lockedRec.stat_shards}
          buildSource={lockedRec.build_source}
          buildNote={lockedRec.build_note}
        />
      )}
      <ComboBoard combos={comboBoard} />
      <CounterItemsPanel items={counterItems} />
      <LaneMatchupPanel matchup={laneMatchup} />
      <GamePlanPanel plan={gamePlan} />
      <DraftSimulatorPanel results={draftSimulation} />
      <DraftForkPanel fork={draftFork} />
      <FeedbackAnalyticsPanel analytics={feedbackAnalytics} />
      <PerformancePanel report={performanceReport} />
      <TeamCompBoard comp={teamComp} />
    </div>
  ) : null;

  return (
    <div className="cs-screen">
      <PhaseHeader
        timeLeftMs={session.time_left_ms}
        lolPhase={session.phase}
        view={phaseView}
        isActing={isActing}
      />

      {/* Lane role drives every recommendation/ban — LCU omits it in
          Blind/Normal/Quickplay, so let the user set it. Hidden for brawl modes
          (ARAM 450, Arena 1700) which have no lanes. */}
      {session.queue_id !== 450 && session.queue_id !== 1700 && onRoleChange && (
        <RoleSelector role={role} source={roleSource} onChange={onRoleChange} />
      )}

      <DraftFlow session={session} />

      <div className="cs-screen__body">
        {/* Sol: Takım */}
        <aside className="cs-screen__team-col">
          <p className="cs-col-label">{t('champSelect.yourTeam')}</p>
          {session.my_team.map(slot => (
            <TeamSlotView key={slot.cell_id} slot={slot} champMap={champMap}
              isLocalPlayer={slot.cell_id === session.my_cell_id} />
          ))}
          <div className="cs-bans">
            {session.my_bans.filter(id => id > 0).map(id => {
              const key = champMap.get(id);
              return (
                <div key={id} className="cs-ban-icon" title={key ?? `#${id}`}>
                  <BanIcon champKey={key} />
                </div>
              );
            })}
          </div>
        </aside>

        {/* Orta: Öneri / Faz içeriği */}
        <main className="cs-screen__center">

          {/* Data-state chips: a subtle footer in the pick dashboard (below), but kept
              on top for the other, less time-critical phases. */}
          {phaseView !== 'pick_acting' && (
            <DataStatusBadges
              recommendations={recommendations}
              laneMatchup={laneMatchup}
              registryReport={dataRegistryReport}
              pipelineReport={pipelineQualityReport}
              trajectoryReport={dataTrajectory}
              qualityReport={draftBrainQualityReport}
              feedbackReport={feedbackObservabilityReport}
            />
          )}

          {/* ── Pick: Sıram — "Glance & Decide" dashboard ── */}
          {phaseView === 'pick_acting' && (
            <div className="cs-dashboard">
              {/* Verdict (sticky, never scrolls) */}
              <div className="cs-dash__verdict">
                <DraftVerdictPanel verdict={draftVerdict} />
                {activeRec && role && (
                  <MetaTrendChip championId={activeRec.champion_id} position={role} />
                )}
                {enemyThreats.length > 0 && (
                  <div className="cs-hover-strip">
                    <span className="cs-ban-hint">{t('champSelect.enemyHovering')}</span>
                    <div className="cs-ban-candidates">
                      {enemyThreats.map(e => (
                        <div key={e.champId} className="cs-ban-candidate">
                          <ChampionIcon championKey={e.champKey ?? ''} size="sm" />
                          <span className="cs-ban-candidate__name">
                            {e.champKey ?? `#${e.champId}`}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>

              {/* Decision: THE pick + its build, always in view */}
              <div className="cs-dash__decision">
                {loading ? (
                  <div className="cs-loading">
                    <LoadingSkeleton variant="hero" />
                    <p className="cs-loading__hint">{t('champSelect.analyzing')}</p>
                  </div>
                ) : activeRec ? (
                  <>
                    {activeIdx === 0 && pickClarity && recommendations.length > 1 && (
                      <div className="cs-pick-clarity">
                        <Badge
                          pill
                          tone={
                            pickClarity.level === 'clear'
                              ? 'teal'
                              : pickClarity.level === 'edge'
                                ? 'gold'
                                : 'warning'
                          }
                        >
                          {pickClarity.level === 'clear'
                            ? t('pickClarity.clear')
                            : pickClarity.level === 'edge'
                              ? t('pickClarity.edge', { b: pickClarity.runnerUpKey })
                              : t('pickClarity.close', {
                                  a: recommendations[0].champion_key,
                                  b: pickClarity.runnerUpKey,
                                })}
                        </Badge>
                      </div>
                    )}
                    <HeroCard
                      key={activeRec.champion_id}
                      rec={activeRec}
                      onHover={onHoverChampion ? () => onHoverChampion(activeRec.champion_id) : undefined}
                      onExpand={() => setSecondaryTab('deepdive')}
                    />
                    {activeRec.core_items.length > 0 && (
                      <BuildSummary
                        coreItems={activeRec.core_items}
                        situationalItems={activeRec.situational_items}
                        primaryRuneTree={activeRec.primary_rune_tree}
                        keystone={activeRec.keystone}
                        championName={activeRec.champion_name || activeRec.champion_key}
                        skillOrder={activeRec.skill_order}
                        summonerSpells={activeRec.summoner_spells}
                        secondaryRunes={activeRec.secondary_runes}
                        statShards={activeRec.stat_shards}
                        buildSource={activeRec.build_source}
                        buildNote={activeRec.build_note}
                      />
                    )}
                  </>
                ) : (
                  <div className="cs-empty">
                    {recError
                      ? <p className="cs-empty__error"><AlertTriangle size={14} /> {recError}</p>
                      : (
                        <>
                          <Compass className="cs-empty__icon" size={40} strokeWidth={1.5} />
                          <p>{t('champSelect.noRecommendation')}</p>
                          <p className="cs-empty__hint">{t('champSelect.noRecommendationCoach')}</p>
                        </>
                      )}
                  </div>
                )}
              </div>

              {/* Alternatives rail (persistent, [1-5] switches the active pick) */}
              <div className="cs-dash__rail">
                {recommendations.length > 1 && (
                  <AlternativesRail
                    recommendations={recommendations}
                    activeIndex={activeIdx}
                    onSelect={setActiveIdx}
                    onHover={onHoverChampion}
                  />
                )}
              </div>

              {/* Secondary depth — tabbed (the only scroll region) */}
              <div className="cs-dash__secondary">
                <SecondaryTabs
                  rec={activeRec}
                  laneMatchup={laneMatchup}
                  counterItems={counterItems}
                  gamePlan={gamePlan}
                  teamComp={teamComp}
                  counterPicks={counterPicks}
                  comboBoard={comboBoard}
                  draftSimulation={draftSimulation}
                  draftFork={draftFork}
                  activeTab={secondaryTab}
                  onTabChange={setSecondaryTab}
                />
              </div>

              {/* Footer: data-quality chips (moved off the top) + hint */}
              <div className="cs-dash__footer">
                <DataStatusBadges
                  recommendations={recommendations}
                  laneMatchup={laneMatchup}
                  registryReport={dataRegistryReport}
                  pipelineReport={pipelineQualityReport}
                  trajectoryReport={dataTrajectory}
                  qualityReport={draftBrainQualityReport}
                  feedbackReport={feedbackObservabilityReport}
                />
                {recommendations.length > 1 && (
                  <span className="cs-keyboard-hint">{t('champSelect.keyboardHintShort')}</span>
                )}
              </div>
            </div>
          )}

          {/* ── Ban: Sıram ── */}
          {phaseView === 'ban_acting' && (
            <div className="cs-ban-view animate-fade-in">
              <p className="cs-ban-title">{t('champSelect.banTitle')}</p>
              {enemyThreats.length > 0 ? (
                <>
                  <p className="cs-ban-hint">{t('champSelect.banHintHover')}</p>
                  <div className="cs-ban-candidates">
                    {enemyThreats.map(t => (
                      <div key={t.champId} className="cs-ban-candidate">
                        <ChampionIcon championKey={t.champKey ?? ''} size="md" />
                        <span className="cs-ban-candidate__name">
                          {t.champKey ?? `#${t.champId}`}
                        </span>
                      </div>
                    ))}
                  </div>
                </>
              ) : (
                <p className="cs-ban-hint">{t('champSelect.banHintNoHover')}</p>
              )}
              <BanSuggestionList suggestions={banSuggestions} />
              <CounterPickBoard picks={counterPicks} />
              <p className="cs-keyboard-hint">{t('champSelect.keyboardHintShort')}</p>
            </div>
          )}

          {/* ── İzleme (ban veya pick) ── */}
          {(phaseView === 'ban_watching' || phaseView === 'pick_watching') && (
            <div className="cs-watch-view animate-fade-in">
              {phaseView === 'pick_watching' && lockedView ? (
                // Already locked while others pick → coach YOUR champion, not "options".
                lockedView
              ) : (
                <>
                  <DraftVerdictPanel verdict={draftVerdict} />
                  <p className="cs-watch-msg">
                    {phaseView === 'ban_watching' ? t('champSelect.watchingBan') : t('champSelect.watchingPick')}
                  </p>
                  {recommendations.length > 0 && (
                    <>
                      <p className="cs-watch-hint">{t('champSelect.yourOptions')}</p>
                      <QuickPickList
                        recommendations={recommendations}
                        activeIndex={activeIdx}
                        onSelect={setActiveIdx}
                      />
                    </>
                  )}
                  <GamePlanPanel plan={gamePlan} />
                  <DraftSimulatorPanel results={draftSimulation} />
                  <DraftForkPanel fork={draftFork} />
                  <FeedbackAnalyticsPanel analytics={feedbackAnalytics} />
                  <PerformancePanel report={performanceReport} />
                  <TeamCompBoard comp={teamComp} />
                </>
              )}
            </div>
          )}

          {/* ── Kilit fazı ── pins to the LOCKED champion (build + game plan). */}
          {phaseView === 'finalization' && (
            <div className="cs-finalization animate-slide-up">
              <p className="cs-finalize-title">{t('champSelect.lockingIn')}</p>
              {lockedView ??
                (session.local_player.champion_id > 0 ? (
                  <LoadingSkeleton rows={1} height={180} />
                ) : null)}
            </div>
          )}

          {/* ── Lobi ── */}
          {phaseView === 'planning' && (
            <div className="cs-planning animate-fade-in">
              <p className="cs-planning-msg">{t('champSelect.planningMsg')}</p>
              <FeedbackAnalyticsPanel analytics={feedbackAnalytics} />
              <PerformancePanel report={performanceReport} />
              <TeamCompBoard comp={teamComp} />
            </div>
          )}
        </main>

        {/* Sağ: Düşman */}
        <aside className="cs-screen__enemy-col">
          <p className="cs-col-label">{t('champSelect.enemyTeam')}</p>
          {session.their_team.map(slot => (
            <TeamSlotView key={slot.cell_id} slot={slot} champMap={champMap} />
          ))}
          <div className="cs-bans">
            {session.their_bans.filter(id => id > 0).map(id => {
              const key = champMap.get(id);
              return (
                <div key={id} className="cs-ban-icon" title={key ?? `#${id}`}>
                  <BanIcon champKey={key} />
                </div>
              );
            })}
          </div>
        </aside>
      </div>
    </div>
  );
};
