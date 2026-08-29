<script setup lang="ts">
/**
 * Die Bildbühne: zeigt das aktuelle Bild bzw. Bildpaar und blendet weich um.
 *
 * NF-16 wörtlich genommen — es sind **immer genau zwei** Ebenen im DOM, egal
 * wie lange die Diashow läuft. Umgeblendet wird ausschließlich über `opacity`
 * und `transform`; beide Eigenschaften bearbeitet der Compositor, ohne das
 * Layout neu zu berechnen. Genau das ist der Unterschied zwischen einer
 * flüssigen und einer ruckelnden Überblendung auf einem alten Tablet (R-02).
 */
import { nextTick, ref, watch } from 'vue'
import { imageUrl } from '@/lib/api'
import type { FitMode, Slide } from '@/lib/types'

const props = defineProps<{
  slide: Slide | null
  fitMode: FitMode
  transitionEnabled: boolean
  transitionMs: number
  kenBurns: boolean
}>()

interface Layer {
  slide: Slide | null
  /** Wechselt bei jedem Belegen — setzt die Ken-Burns-Animation neu an. */
  generation: number
}

const layers = ref<[Layer, Layer]>([
  { slide: null, generation: 0 },
  { slide: null, generation: 0 },
])
const front = ref(0)
let generation = 0

watch(
  () => props.slide,
  async (next) => {
    if (!next) {
      layers.value[0].slide = null
      layers.value[1].slide = null
      return
    }
    // Erstes Bild ohne Überblendung — sonst startet die App mit einem
    // Schwarzbild, das sich langsam aufhellt.
    const isFirst = layers.value[front.value].slide === null
    const back = isFirst ? front.value : 1 - front.value

    layers.value[back] = { slide: next, generation: ++generation }
    await nextTick()
    front.value = back
  },
  { immediate: true },
)

function idsOf(slide: Slide | null): string[] {
  if (!slide) return []
  return slide.kind === 'single' ? [slide.id] : [slide.left, slide.right]
}
</script>

<template>
  <div class="stage">
    <div
      v-for="(layer, index) in layers"
      :key="index"
      class="layer"
      :class="{ visible: index === front && layer.slide }"
      :style="{
        transitionDuration: props.transitionEnabled ? `${props.transitionMs}ms` : '0ms',
      }"
      aria-hidden="true"
    >
      <div v-if="layer.slide" class="frame" :class="{ pair: layer.slide.kind === 'pair' }">
        <img
          v-for="id in idsOf(layer.slide)"
          :key="`${layer.generation}-${id}`"
          class="photo"
          :class="[props.fitMode, { 'ken-burns': props.kenBurns }]"
          :src="imageUrl(id)"
          alt=""
          decoding="async"
          draggable="false"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.stage {
  position: absolute;
  inset: 0;
  background: var(--ss-bg);
  overflow: hidden;
}

.layer {
  position: absolute;
  inset: 0;
  opacity: 0;
  transition-property: opacity;
  transition-timing-function: ease-in-out;
  /* Eigene Compositor-Ebene, damit das Umblenden die GPU nicht zwingt,
     die Textur bei jedem Bild neu hochzuladen. */
  will-change: opacity;
}

.layer.visible {
  opacity: 1;
}

.frame {
  position: absolute;
  inset: 0;
  display: flex;
}

/* Paar-Modus: zwei Hochformatbilder nebeneinander (FA-08). Der schmale Spalt
   trennt sie sichtbar, ohne wie ein Rahmen zu wirken. */
.frame.pair {
  gap: 2px;
}

.frame.pair .photo {
  width: 50%;
  height: 100%;
}

.photo {
  width: 100%;
  height: 100%;
  display: block;
  user-select: none;
  -webkit-user-drag: none;
}

/* FA-05: beide Modi ohne Verzerrung. */
.photo.contain {
  object-fit: contain;
}

.photo.cover {
  object-fit: cover;
}

/* FA-10: langsames Zoomen und Schwenken. Rein über `transform` — kein
   Layout-Reflow, damit NF-16 auch mit Effekt eingehalten bleibt. */
.photo.ken-burns {
  animation: ken-burns 40s ease-out forwards;
  transform-origin: center center;
}

@keyframes ken-burns {
  from {
    transform: scale(1) translate3d(0, 0, 0);
  }
  to {
    transform: scale(1.08) translate3d(-1.2%, -0.8%, 0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .photo.ken-burns {
    animation: none;
  }
}
</style>
