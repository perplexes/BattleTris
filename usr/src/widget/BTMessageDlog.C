/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTMessageDlog.C                                     */
/*    DATE: Tue Apr  5 02:44:32 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTMessageDlog.H"
#include "BTWidget.H"

BTMessageDlog::BTMessageDlog(BTWidget *parent, const char *message)
{
	Visual *visual;
	Pixmap bg_pixmap;
	Pixmap brdr_pixmap;
	Colormap colormap;
	int depth;

	Arg args[10];
	int i = 0;

	XmString str = XmStringCreateSimple((char *)message);

	XtVaGetValues(*parent,
	    XmNvisual, &visual,
	    XmNbackgroundPixmap, &bg_pixmap,
	    XmNborderPixmap, &brdr_pixmap,
	    XmNcolormap, &colormap,
	    XmNdepth, &depth,
	    NULL);

	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNdialogStyle, XmDIALOG_FULL_APPLICATION_MODAL); i++;
	XtSetArg(args[i], XmNmessageString, str); i++;
	XtSetArg(args[i], XmNdefaultButtonType, XmDIALOG_OK_BUTTON); i++;
	XtSetArg(args[i], XmNvisual, visual); i++;
	XtSetArg(args[i], XmNbackgroundPixmap, bg_pixmap); i++;
	XtSetArg(args[i], XmNborderPixmap, brdr_pixmap); i++;
	XtSetArg(args[i], XmNcolormap, colormap); i++;
	XtSetArg(args[i], XmNdepth, depth); i++;

	me_ = XmCreateWarningDialog(*parent, (char *)"BTMessageDlog", args, i);
	XmStringFree(str);

	XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_HELP_BUTTON));
	XtUnmanageChild(XmMessageBoxGetChild(me_, XmDIALOG_CANCEL_BUTTON));

	XtAddCallback(me_, XmNokCallback, accept_cb, (XtPointer) me_);
	XtManageChild(me_);
}

void BTMessageDlog::accept_cb(Widget widget, XtPointer data, XtPointer cbs)
{
  Widget me = (Widget) data;

  XtUnmanageChild(me);
  XtDestroyWidget(me);
}
