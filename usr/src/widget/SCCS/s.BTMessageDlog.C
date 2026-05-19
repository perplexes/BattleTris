h19803
s 00031/00027/00028
d D 1.2 01/10/21 19:25:16 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:14 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTMessageDlog.C
c Name history : 1 0 src/widget/BTMessageDlog.C
e
s 00055/00000/00000
d D 1.1 01/10/20 13:35:13 bmc 1 0
c date and time created 01/10/20 13:35:13 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTMessageDlog.C                                     */
/*    DATE: Tue Apr  5 02:44:32 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTMessageDlog.H"
#include "BTWidget.H"

D 3
BTMessageDlog::BTMessageDlog(BTWidget *parent, char *message)
E 3
I 3
BTMessageDlog::BTMessageDlog(BTWidget *parent, const char *message)
E 3
{
D 3
  Visual *visual;
  Pixmap bg_pixmap;
  Pixmap brdr_pixmap;
  Colormap colormap;
  int depth;
E 3
I 3
	Visual *visual;
	Pixmap bg_pixmap;
	Pixmap brdr_pixmap;
	Colormap colormap;
	int depth;
E 3

D 3
  Arg args[10];
  int i = 0;
E 3
I 3
	Arg args[10];
	int i = 0;
E 3

D 3
  XmString str = XmStringCreateSimple(message);
E 3
I 3
	XmString str = XmStringCreateSimple((char *)message);
E 3

D 3
  XtVaGetValues(*parent, XmNvisual, &visual, XmNbackgroundPixmap, &bg_pixmap,
		XmNborderPixmap, &brdr_pixmap, XmNcolormap, &colormap,
		XmNdepth, &depth, NULL);
E 3
I 3
	XtVaGetValues(*parent,
	    XmNvisual, &visual,
	    XmNbackgroundPixmap, &bg_pixmap,
	    XmNborderPixmap, &brdr_pixmap,
	    XmNcolormap, &colormap,
	    XmNdepth, &depth,
	    NULL);
E 3

D 3
  XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
  XtSetArg(args[i], XmNdialogStyle, XmDIALOG_FULL_APPLICATION_MODAL); i++;
  XtSetArg(args[i], XmNmessageString, str); i++;
  XtSetArg(args[i], XmNdefaultButtonType, XmDIALOG_OK_BUTTON); i++;
  XtSetArg(args[i], XmNvisual, visual); i++;
  XtSetArg(args[i], XmNbackgroundPixmap, bg_pixmap); i++;
  XtSetArg(args[i], XmNborderPixmap, brdr_pixmap); i++;
  XtSetArg(args[i], XmNcolormap, colormap); i++;
  XtSetArg(args[i], XmNdepth, depth); i++;
E 3
I 3
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNdialogStyle, XmDIALOG_FULL_APPLICATION_MODAL); i++;
	XtSetArg(args[i], XmNmessageString, str); i++;
	XtSetArg(args[i], XmNdefaultButtonType, XmDIALOG_OK_BUTTON); i++;
	XtSetArg(args[i], XmNvisual, visual); i++;
	XtSetArg(args[i], XmNbackgroundPixmap, bg_pixmap); i++;
	XtSetArg(args[i], XmNborderPixmap, brdr_pixmap); i++;
	XtSetArg(args[i], XmNcolormap, colormap); i++;
	XtSetArg(args[i], XmNdepth, depth); i++;
E 3

D 3
  me_ = XmCreateWarningDialog(*parent, "BTMessageDlog", args, i);
  XmStringFree(str);
E 3
I 3
	me_ = XmCreateWarningDialog(*parent, (char *)"BTMessageDlog", args, i);
	XmStringFree(str);
E 3

D 3
  XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_HELP_BUTTON));
  XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_CANCEL_BUTTON));
E 3
I 3
	XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_HELP_BUTTON));
	XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_CANCEL_BUTTON));
E 3

D 3
  XtAddCallback(me_, XmNokCallback, accept_cb, (XtPointer) me_);
  XtManageChild(me_);
E 3
I 3
	XtAddCallback(me_, XmNokCallback, accept_cb, (XtPointer) me_);
	XtManageChild(me_);
E 3
}

void BTMessageDlog::accept_cb(Widget widget, XtPointer data, XtPointer cbs)
{
  Widget me = (Widget) data;

  XtUnmanageChild(me);
  XtDestroyWidget(me);
}
E 1
