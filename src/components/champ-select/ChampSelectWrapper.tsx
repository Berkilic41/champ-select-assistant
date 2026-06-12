import React, { useEffect, useCallback, useState } from 'react';
import { invoke } from '../../lib/host';
import { useTranslation } from 'react-i18next';
import { useChampSelect } from '../../hooks/useChampSelect';
import { detectPhaseView } from '../../hooks/useChampSelectPhase';
import { ChampSelectScreen } from './ChampSelectScreen';
import { BanSuggestion } from '../../types/recommendation';
import type { DataSourceRegistryReport } from '../../types/generated/DataSourceRegistryReport';
import type { DraftBrainQualityReport } from '../../types/generated/DraftBrainQualityReport';
import type { FeedbackAnalytics } from '../../types/generated/FeedbackAnalytics';
import type { FeedbackObservabilityReport } from '../../types/generated/FeedbackObservabilityReport';
import type { PerformanceReport } from '../../types/generated/PerformanceReport';
import type { DraftSimResult } from '../../types/generated/DraftSimResult';
import type { PipelineQualityReport } from '../../types/generated/PipelineQualityReport';
import type { DataTrajectoryView } from '../../types/generated/DataTrajectoryView';
import './ChampSelectWrapper.css';

interface ChampionRecord {
  champion_id: number;
  key: string;
}

interface Props {
  summonerName: string;
  port: number;
  addToast: (message: string, type?: 'info' | 'success' | 'warning' | 'error') => void;
}

export const ChampSelectWrapper: React.FC<Props> = ({ addToast }) => {
  const { t } = useTranslation();
  const [puuid, setPuuid] = useState<string>('');
  const { session, recommendations, lockedAnalysis, gamePlan, counterPicks, teamComp, comboBoard, draftVerdict, counterItems, laneMatchup, role, roleSource, setRole, loading, error } = useChampSelect(puuid);
  const [champMap, setChampMap] = useState<Map<number, string>>(new Map());
  const [banSuggestions, setBanSuggestions] = useState<BanSuggestion[]>([]);
  const [dataRegistryReport, setDataRegistryReport] = useState<DataSourceRegistryReport | null>(null);
  const [pipelineQualityReport, setPipelineQualityReport] = useState<PipelineQualityReport | null>(null);
  const [dataTrajectory, setDataTrajectory] = useState<DataTrajectoryView | null>(null);
  const [draftBrainQualityReport, setDraftBrainQualityReport] = useState<DraftBrainQualityReport | null>(null);
  const [feedbackObservabilityReport, setFeedbackObservabilityReport] = useState<FeedbackObservabilityReport | null>(null);
  const [feedbackAnalytics, setFeedbackAnalytics] = useState<FeedbackAnalytics | null>(null);
  const [performanceReport, setPerformanceReport] = useState<PerformanceReport | null>(null);
  const [draftSimulation, setDraftSimulation] = useState<DraftSimResult[] | null>(null);

  // Resolve the active player's puuid once on mount. Empty string is acceptable
  // — backend gracefully returns empty mastery/stats — but causes empty recs.
  useEffect(() => {
    invoke<string | null>('get_active_summoner_puuid')
      .then((p) => setPuuid(p ?? ''))
      .catch(() => setPuuid(''));
  }, []);

  useEffect(() => {
    invoke<ChampionRecord[]>('get_champions')
      .then((records) => setChampMap(new Map(records.map((r) => [r.champion_id, r.key]))))
      .catch(() => setChampMap(new Map()));
  }, []);

  const refreshQualityReports = useCallback(() => {
    invoke<DraftBrainQualityReport>('get_draft_brain_quality_report')
      .then(setDraftBrainQualityReport)
      .catch(() => setDraftBrainQualityReport(null));
    invoke<DataSourceRegistryReport>('get_data_source_registry')
      .then(setDataRegistryReport)
      .catch(() => setDataRegistryReport(null));
    invoke<PipelineQualityReport>('get_pipeline_quality_report')
      .then(setPipelineQualityReport)
      .catch(() => setPipelineQualityReport(null));
    invoke<DataTrajectoryView>('get_data_trajectory')
      .then(setDataTrajectory)
      .catch(() => setDataTrajectory(null));
    invoke<FeedbackObservabilityReport>('get_feedback_observability')
      .then(setFeedbackObservabilityReport)
      .catch(() => setFeedbackObservabilityReport(null));
    invoke<FeedbackAnalytics>('get_feedback_analytics', { windowDays: 7 })
      .then(setFeedbackAnalytics)
      .catch(() => setFeedbackAnalytics(null));
  }, []);

  useEffect(() => {
    refreshQualityReports();
  }, [session?.phase, recommendations.length, refreshQualityReports]);

  useEffect(() => {
    if (!puuid) {
      setPerformanceReport(null);
      return;
    }
    invoke<PerformanceReport>('get_performance_report', { puuid })
      .then(setPerformanceReport)
      .catch(() => setPerformanceReport(null));
  }, [puuid]);

  useEffect(() => {
    if (!session || recommendations.length === 0) {
      setDraftSimulation(null);
      return;
    }
    const candidateIds = recommendations.slice(0, 5).map((rec) => rec.champion_id);
    invoke<DraftSimResult[]>('get_draft_simulation', {
      sessionJson: session,
      candidateIds,
    })
      .then(setDraftSimulation)
      .catch(() => setDraftSimulation(null));
  }, [session, recommendations]);

  // Fetch ban suggestions when it is the local player's ban turn.
  useEffect(() => {
    if (!session || session.action_type !== 'ban') {
      setBanSuggestions([]);
      return;
    }
    invoke<BanSuggestion[]>('get_ban_suggestions', {
      sessionJson: session,
      puuid,
    }).then(setBanSuggestions).catch(() => setBanSuggestions([]));
    // Re-fetch on role change too — bans key off the lane meta + pool counters.
  }, [session?.action_type, session?.my_bans.length, session?.their_bans.length, session?.local_player.assigned_position, puuid]);

  // Surface recommendation errors as toasts (non-blocking).
  useEffect(() => {
    if (error) addToast(error, 'warning');
  }, [error, addToast]);

  const handleHoverChampion = useCallback(
    async (championId: number) => {
      try {
        await invoke<void>('hover_champion', { championId });
      } catch (err) {
        addToast(t('champSelect.hoverFailed', { err: String(err) }), 'error');
      }
    },
    [addToast, t],
  );

  if (!session) {
    return <div className="status-placeholder">{t('champSelect.waiting')}</div>;
  }

  const phaseView = detectPhaseView(session);

  return (
    <>
      {session.queue_id === 450 && <span className="aram-badge">ARAM</span>}
      {session.queue_id === 1700 && <span className="aram-badge">ARENA</span>}
      <ChampSelectScreen
        session={session}
        role={role}
        roleSource={roleSource}
        onRoleChange={setRole}
        recommendations={recommendations}
        lockedRec={lockedAnalysis}
        gamePlan={gamePlan}
        counterPicks={counterPicks}
        teamComp={teamComp}
        comboBoard={comboBoard}
        draftVerdict={draftVerdict}
        counterItems={counterItems}
        laneMatchup={laneMatchup}
        dataRegistryReport={dataRegistryReport}
        pipelineQualityReport={pipelineQualityReport}
        dataTrajectory={dataTrajectory}
        draftBrainQualityReport={draftBrainQualityReport}
        feedbackObservabilityReport={feedbackObservabilityReport}
        feedbackAnalytics={feedbackAnalytics}
        draftSimulation={draftSimulation}
        performanceReport={performanceReport}
        champMap={champMap}
        loading={loading}
        phaseView={phaseView}
        recError={error}
        banSuggestions={banSuggestions}
        onFeedbackSubmitted={refreshQualityReports}
        onHoverChampion={handleHoverChampion}
      />
    </>
  );
};
