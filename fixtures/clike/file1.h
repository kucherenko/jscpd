/*
 * Copy the size of snapshot frame "sn" to frame "fr".  Do the same for all
 * following frames and children.
 * Returns a pointer to the old current window, or NULL.
 */
static win_T *restore_snapshot_rec(frame_T *sn, frame_T *fr)
{
  win_T       *wp = NULL;
  win_T       *wp2;

  fr->fr_height = sn->fr_height;
  fr->fr_width = sn->fr_width;
  if (fr->fr_layout == FR_LEAF) {
    frame_new_height(fr, fr->fr_height, FALSE, FALSE);
    frame_new_width(fr, fr->fr_width, FALSE, FALSE);
    wp = sn->fr_win;
  }
  return wp;
}
