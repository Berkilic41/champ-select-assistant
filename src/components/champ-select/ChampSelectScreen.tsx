import React, { useState } from 'react';
import { AlertTriangle } from 'lucide-react';
import { ChampSelectSession, Recommendation, BanSuggestion, EnemyPoolSummary } from '../../types/recommendation';
import { PhaseView } from '../../hooks/useChampSelectPhase';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { PhaseHeader } from './PhaseHeader';
import { HeroCard } from './HeroCard';
import { QuickPickList } from './QuickPickList';
import { TeamSlotView } from './TeamSlotView';
import { BuildSummary } from './BuildSummary';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
import { ChampionIcon } from '../shared/ChampionIcon';
import { BanSuggestionList } from './BanSuggestionList';
import { champIconUrl } from '../../lib/ddragon';
import './ChampSelectScreen.css';

interface Props {
  session: ChampSelectSession;
  recommendations: Recommendation[];
  champMap: Map<number, string>;
  loading: boolean;
  phaseView: PhaseView;
  recError?: string | null;
  banSuggestions: BanSuggestion[];
  enemyPools?: EnemyPoolSummary[];
  onHoverChampion?: (championId: number) => void;
}

export const ChampSelectScreen: React.FC<Props> = ({
  session, recommendations, champMap, loading, phaseView, recError, banSuggestions,
  enemyPools = [],
  onHoverChampion,
}) => {
  const [activeIdx, setActiveIdx] = useState(0);
  const isActing = phaseView === 'pick_acting' || phaseView === 'ban_acting';
  const activeRec = recommendations[activeIdx];

  useKeyboardShortcuts(
    recommendations,
    isActing,
    onHoverChampion ?? (() => {}),
    activeIdx,
  );

  // Enemy team's current hover targets — shown during ban phase
  const enemyThreats = session.their_team
    .filter(s => s.intent_champion_id > 0)
    .map(s => ({
      champId: s.intent_champion_id,
      champKey: champMap.get(s.intent_champion_id),
    }));

  return (
    <div className="cs-screen">
      <PhaseHeader
        timeLeftMs={session.time_left_ms}
        lolPhase={session.phase}
        view={phaseView}
        isActing={isActing}
      />

      <div className="cs-screen__body">
        {/* Sol: Takım */}
        <aside className="cs-screen__team-col">
          <p className="cs-col-label">Takımın</p>
          {session.my_team.map(slot => (
            <TeamSlotView key={slot.cell_id} slot={slot} champMap={champMap}
              isLocalPlayer={slot.cell_id === session.my_cell_id} />
          ))}
          <div className="cs-bans">
            {session.my_bans.filter(id => id > 0).map(id => {
              const key = champMap.get(id);
              return (
                <div key={id} className="cs-ban-icon" title={key ?? `#${id}`}>
                  {key
                    ? <img src={champIconUrl(key)} alt={key} className="cs-ban-img" />
                    : <div className="cs-ban-img cs-ban-img--unknown" />}
                </div>
              );
            })}
          </div>
        </aside>

        {/* Orta: Öneri / Faz içeriği */}
        <main className="cs-screen__center">

          {/* ── Pick: Sıram ── */}
          {phaseView === 'pick_acting' && (
            <>
              {loading ? (
                <LoadingSkeleton rows={1} height={180} />
              ) : activeRec ? (
                <HeroCard
                  rec={activeRec}
                  onHover={onHoverChampion ? () => onHoverChampion(activeRec.champion_id) : undefined}
                />
              ) : (
                <div className="cs-empty">
                  {recError
                    ? <p className="cs-empty__error"><AlertTriangle size={14} /> {recError}</p>
                    : <p>Öneri için "Maç geçmişini yükle" butonuna tıkla</p>}
                </div>
              )}
              {recommendations.length > 0 && (
                <QuickPickList
                  recommendations={recommendations}
                  activeIndex={activeIdx}
                  onSelect={setActiveIdx}
                />
              )}
              {activeRec && activeRec.core_items.length > 0 && (
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
                />
              )}
            </>
          )}

          {/* ── Ban: Sıram ── */}
          {phaseView === 'ban_acting' && (
            <div className="cs-ban-view animate-fade-in">
              <p className="cs-ban-title">BAN SIRAN</p>
              {enemyThreats.length > 0 ? (
                <>
                  <p className="cs-ban-hint">Rakip hover ediyor — potansiyel ban hedefleri:</p>
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
                <p className="cs-ban-hint">
                  Rakip henüz hover etmiyor. Kompo eksikliğini karşılayan şampiyonları banlama önerilir.
                </p>
              )}
              <BanSuggestionList suggestions={banSuggestions} enemyPools={enemyPools} />
              <p className="cs-keyboard-hint">[1-5] Öneri seç  ·  Enter = Hover uygula</p>
            </div>
          )}

          {/* ── İzleme (ban veya pick) ── */}
          {(phaseView === 'ban_watching' || phaseView === 'pick_watching') && (
            <div className="cs-watch-view animate-fade-in">
              <p className="cs-watch-msg">
                {phaseView === 'ban_watching' ? 'Takım banlıyor...' : 'Takım seçiyor...'}
              </p>
              {recommendations.length > 0 && (
                <>
                  <p className="cs-watch-hint">Seçeneklerin:</p>
                  <QuickPickList
                    recommendations={recommendations}
                    activeIndex={activeIdx}
                    onSelect={setActiveIdx}
                  />
                </>
              )}
            </div>
          )}

          {/* ── Kilit fazı ── */}
          {phaseView === 'finalization' && (
            <div className="cs-finalization animate-slide-up">
              <p className="cs-finalize-title">Seçim kilitlendi — Build planı:</p>
              {session.local_player.champion_id > 0 && activeRec && (
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
                />
              )}
            </div>
          )}

          {/* ── Lobi ── */}
          {phaseView === 'planning' && (
            <div className="cs-planning animate-fade-in">
              <p className="cs-planning-msg">
                Lobi'desin — Champion Select'e girince öneriler otomatik görünecek
              </p>
            </div>
          )}
        </main>

        {/* Sağ: Düşman */}
        <aside className="cs-screen__enemy-col">
          <p className="cs-col-label">Düşman</p>
          {session.their_team.map(slot => (
            <TeamSlotView key={slot.cell_id} slot={slot} champMap={champMap} />
          ))}
          <div className="cs-bans">
            {session.their_bans.filter(id => id > 0).map(id => {
              const key = champMap.get(id);
              return (
                <div key={id} className="cs-ban-icon" title={key ?? `#${id}`}>
                  {key
                    ? <img src={champIconUrl(key)} alt={key} className="cs-ban-img" />
                    : <div className="cs-ban-img cs-ban-img--unknown" />}
                </div>
              );
            })}
          </div>
        </aside>
      </div>
    </div>
  );
};
