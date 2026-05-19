/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTChallenge.C                                       */
/*    DATE: Wed Apr 20 17:56:27 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTNetManager.H"
#include "BTPlayer.H"
#include "BTNetworkEntry.H"
#include "BTChallenge.H"
#include "BTDisplay.H"
#include "BTPixmap.H"
#include "BTComputer.H"

#define BT_CHALLENGE_FRAC_BASE 800
#define BT_CHAL_XFRAC 1
#define BT_CHAL_YFRAC 8 / 7

#define BT_CHALLENGE_DRAWING_AREA_X1 20 * BT_CHAL_XFRAC
#define BT_CHALLENGE_DRAWING_AREA_Y1 20 * BT_CHAL_YFRAC
#define BT_CHALLENGE_DRAWING_AREA_X2 580 * BT_CHAL_XFRAC
#define BT_CHALLENGE_DRAWING_AREA_Y2 130 * BT_CHAL_YFRAC

#define BT_CHALLENGE_USER_LIST_X1 20 * BT_CHAL_XFRAC
#define BT_CHALLENGE_USER_LIST_Y1 20 * BT_CHAL_YFRAC
#define BT_CHALLENGE_USER_LIST_X2 400 * BT_CHAL_XFRAC
#define BT_CHALLENGE_USER_LIST_Y2 500 * BT_CHAL_YFRAC

#define BT_CHALLENGE_COMPUTER_FRAME_X1 20 * BT_CHAL_XFRAC
#define BT_CHALLENGE_COMPUTER_FRAME_Y1 520 * BT_CHAL_YFRAC
#define BT_CHALLENGE_COMPUTER_FRAME_X2 400 * BT_CHAL_XFRAC
#define BT_CHALLENGE_COMPUTER_FRAME_Y2 680 * BT_CHAL_YFRAC

#define	BT_CHALLENGE_COMPUTER_FRAME_YOFFSET	8
#define	BT_CHALLENGE_COMPUTER_FRAME_XOFFSET	7

#define BT_CHALLENGE_USER_INFO_X1 440 * BT_CHAL_XFRAC
#define BT_CHALLENGE_USER_INFO_Y1 320 * BT_CHAL_YFRAC
#define BT_CHALLENGE_USER_INFO_X2 780 * BT_CHAL_XFRAC
#define BT_CHALLENGE_USER_INFO_Y2 680 * BT_CHAL_YFRAC

#define BT_CHALLENGE_CHALLENGE_BUTTON_X1 450 * BT_CHAL_XFRAC
#define BT_CHALLENGE_CHALLENGE_BUTTON_Y1 170 * BT_CHAL_YFRAC
#define BT_CHALLENGE_CHALLENGE_BUTTON_X2 570 * BT_CHAL_XFRAC
#define BT_CHALLENGE_CHALLENGE_BUTTON_Y2 220 * BT_CHAL_YFRAC

#define BT_CHALLENGE_UPDATE_BUTTON_X1 640 * BT_CHAL_XFRAC
#define BT_CHALLENGE_UPDATE_BUTTON_Y1 170 * BT_CHAL_YFRAC
#define BT_CHALLENGE_UPDATE_BUTTON_X2 760 * BT_CHAL_XFRAC
#define BT_CHALLENGE_UPDATE_BUTTON_Y2 220 * BT_CHAL_YFRAC

#define BT_CHALLENGE_CANCEL_BUTTON_X1 545 * BT_CHAL_XFRAC
#define BT_CHALLENGE_CANCEL_BUTTON_Y1 240 * BT_CHAL_YFRAC
#define BT_CHALLENGE_CANCEL_BUTTON_X2 665 * BT_CHAL_XFRAC
#define BT_CHALLENGE_CANCEL_BUTTON_Y2 290 * BT_CHAL_YFRAC

#define BT_CHALLENGE_LOGO_X1 540 * BT_CHAL_XFRAC
#define BT_CHALLENGE_LOGO_Y1 30 * BT_CHAL_YFRAC

BTChallenge::BTChallenge(BTWidget *parent, BTNetManager *netMgr,
    BTPixmap *icon)
:	netMgr_(netMgr), selection_(0),
	cursor_(XCreateFontCursor(XtDisplay(parent->getWidget()), XC_watch)),
	form_(parent, "BTChallenge",
	    BT_CHALLENGE_WIDTH, BT_CHALLENGE_HEIGHT, BT_CHALLENGE_FRAC_BASE),
	drawing_area_(&form_, "drawing_area", icon, icon->width_,
	    icon->height_),
	user_info_(&form_, "user_info", " "),
	user_list_(&form_, "user_list", NULL, 0),
	challenge_button_(&form_, "challenge_button", "Challenge"),
	update_button_(&form_, "update_button", "Update"),
	cancel_button_(&form_, "cancel_button", "Cancel"),
	computer_frame_(&form_, "computer_frame"),
	computer_label_(&computer_frame_, "computer_label", "Play Computer"),
	computer_form_(&computer_frame_, "computer_form", 0,
	    0, BT_CHALLENGE_FRAC_BASE),
	computer_button_(&computer_form_, "computer_button",
	    "Play Computer"),
	computer_toggle_(&computer_form_, "computer_toggle",
	    "Available for challenges", 1),
	avail_(1),
	ernie_slider_(&computer_form_, "ernie_slider", BTComputer::nLevels()),
	level_(-1)
{
	avail_ = 1;
	level_ = -1;

	form_.placeChild(&drawing_area_, BT_CHALLENGE_LOGO_X1,
	    BT_CHALLENGE_LOGO_Y1);
	drawing_area_.manage();

	form_.placeChild(&user_info_,
	    BT_CHALLENGE_USER_INFO_X1, BT_CHALLENGE_USER_INFO_Y1,
	    BT_CHALLENGE_USER_INFO_X2, BT_CHALLENGE_USER_INFO_Y2);
	user_info_.manage();

	form_.placeChild(&user_list_,
	    BT_CHALLENGE_USER_LIST_X1, BT_CHALLENGE_USER_LIST_Y1,
	    BT_CHALLENGE_USER_LIST_X2, BT_CHALLENGE_USER_LIST_Y2);
	user_list_.manage();

	user_list_.addDefActionCallback(handleSelection_CB, this);
	user_list_.addBrowseSelCallback(handleSelection_CB, this);

	user_list_.selectPos(1, 1);

	form_.placeChild( &challenge_button_,
	    BT_CHALLENGE_CHALLENGE_BUTTON_X1,
	    BT_CHALLENGE_CHALLENGE_BUTTON_Y1,
	    BT_CHALLENGE_CHALLENGE_BUTTON_X2,
	    BT_CHALLENGE_CHALLENGE_BUTTON_Y2);
	challenge_button_.manage();
	challenge_button_.addActivateCallback(handleChallenge_CB, this);

	form_.placeChild(&update_button_,
	    BT_CHALLENGE_UPDATE_BUTTON_X1, BT_CHALLENGE_UPDATE_BUTTON_Y1,
	    BT_CHALLENGE_UPDATE_BUTTON_X2, BT_CHALLENGE_UPDATE_BUTTON_Y2);
	update_button_.manage();
	update_button_.addActivateCallback(handleUpdate_CB, this);

	form_.placeChild(&cancel_button_,
	    BT_CHALLENGE_CANCEL_BUTTON_X1, BT_CHALLENGE_CANCEL_BUTTON_Y1,
	    BT_CHALLENGE_CANCEL_BUTTON_X2, BT_CHALLENGE_CANCEL_BUTTON_Y2);
	cancel_button_.manage(); // BTStartup takes care of this callback

	form_.placeChild(&computer_frame_,
	    BT_CHALLENGE_COMPUTER_FRAME_X1, BT_CHALLENGE_COMPUTER_FRAME_Y1,
	    BT_CHALLENGE_COMPUTER_FRAME_X2, BT_CHALLENGE_COMPUTER_FRAME_Y2);

	XtVaSetValues((Widget)computer_label_,
	    XmNalignment, XmALIGNMENT_BEGINNING,
	    XmNchildType, XmFRAME_TITLE_CHILD,
	    NULL);

	computer_label_.manage();
	computer_frame_.manage();

	handleLevel();
	XtVaSetValues((Widget)computer_button_,
	    XmNtopAttachment, XmATTACH_FORM,
	    XmNtopOffset, BT_CHALLENGE_COMPUTER_FRAME_YOFFSET,
	    XmNleftAttachment, XmATTACH_FORM,
	    XmNleftOffset, BT_CHALLENGE_COMPUTER_FRAME_XOFFSET,
	    XmNrightAttachment, XmATTACH_FORM,
	    XmNrightOffset, BT_CHALLENGE_COMPUTER_FRAME_XOFFSET,
	    NULL);

	computer_button_.manage(); // BTStartup takes care of this callback

	XtVaSetValues((Widget)ernie_slider_,
	    XmNleftAttachment, XmATTACH_FORM,
	    XmNleftOffset, BT_CHALLENGE_COMPUTER_FRAME_XOFFSET,
	    XmNrightAttachment, XmATTACH_FORM,
	    XmNrightOffset, BT_CHALLENGE_COMPUTER_FRAME_XOFFSET,
	    XmNtopAttachment, XmATTACH_WIDGET,
	    XmNtopWidget, (Widget)computer_button_,
	    NULL);

	ernie_slider_.addChangeCallback(handleLevel_CB, this);
	ernie_slider_.manage();

	XtVaSetValues((Widget)computer_toggle_,
	    XmNleftAttachment, XmATTACH_FORM,
	    XmNleftOffset, BT_CHALLENGE_COMPUTER_FRAME_XOFFSET,
	    XmNbottomAttachment, XmATTACH_FORM,
	    XmNbottomOffset, BT_CHALLENGE_COMPUTER_FRAME_YOFFSET,
	    NULL);

	computer_toggle_.addToggleCallback(handleToggle_CB, this);
	computer_toggle_.manage();
	computer_form_.manage();
}

void BTChallenge::show()
{
  XUndefineCursor(XtDisplay(form_), XtWindow(form_));
  form_.manage();
  netMgr_->plyupdate();
  BTChallenge::update();
}

void BTChallenge::hide()
{
  form_.unmanage();
}

void BTChallenge::_chaltimeout_(void *data, unsigned long *)
{
  BTChallenge *t = (BTChallenge *) data;
  XUndefineCursor(XtDisplay(t->form_),
		  XtWindow(t->form_));
  DISPLAY->flush();
}

void
BTChallenge::handleLevel()
{
	char label[256];

	if (level_ == ernie_slider_.value_)
		return;

	level_ = ernie_slider_.value_;

 	sprintf(label, "Play %s Ernie", BTComputer::levelName(level_));

	computer_button_.setLabel(label);
	computer_button_.alignCenter();
}

void BTChallenge::handleToggle()
{
  avail_ = computer_toggle_.state_;
}

void BTChallenge::handleSelection()
{
  selection_ = user_list_.last_selection_;

  if(selection_ >= netMgr_->netlen())
    return;

  BTNetworkEntry *netEntry = netMgr_->netentry(selection_);
  BTPlayer *player = netMgr_->plyentry(netEntry->userName_);

  if(player)
    user_info_.setText(player->formatInfo());
}

void BTChallenge::handleUpdate()
{
  XDefineCursor(XtDisplay(form_), XtWindow(form_), cursor_);
  BTChallenge::update();
  XUndefineCursor(XtDisplay(form_), XtWindow(form_));
}

void BTChallenge::handleChallenge()
{
  XDefineCursor(XtDisplay(form_), XtWindow(form_), cursor_);
  DISPLAY->flush();

  BTNetworkEntry *entry = netMgr_->netentry(selection_);

  if(entry != 0)
    netMgr_->challenge(entry);

  XUndefineCursor(XtDisplay(form_), XtWindow(form_));
  DISPLAY->flush();
}

void BTChallenge::update()
{
  netMgr_->netupdate();
  user_list_.setList(netMgr_->netbuf(), netMgr_->netlen());
  selection_ = 0;

  if(selection_ >= netMgr_->netlen())
    return;

  BTNetworkEntry *netEntry = netMgr_->netentry(selection_);
  BTPlayer *player = netMgr_->plyentry(netEntry->userName_);

  if(player)
    user_info_.setText(player->formatInfo());
}
