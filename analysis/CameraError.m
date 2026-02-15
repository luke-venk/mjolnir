% ==========================================================================================
% Alexander Lozano 2/12/26
% This code will quantify the error of cameras looking downfield at a
% throwing sport implement
%
% KEY ASSUMPTIONS
%
% The error quantified here is one pixel deep, i.e we assume there is 1 pixel of the
% implement showing, we know its exact location to the error of that pixel
% bounding box
%
% We will take the smallest possible error of all three cameras in the limit
% of pixel bounding boxes to be the error of that pixel. Essentially we
% assume he cameras do not communicate. In real life we opuld likely have
% sub-pixel accuracy due to multiple filming locations, however, if 2 or 3
% cameras pixel density lined up perfectly it would only be possible to use
% the smallest pixel for accuracy, we assume this worst case
%
% Glare, noise and other forms of error are not included in this analysis
%
% Assume Focal length is small enough, lens distortion is not applicable.
% Also assume error is quantified rectilinearly, i.e pinhole camera
% approximation for each pixel. The error is technically a function of
% radial distance from the camera in polar coords, but for ease of
% programming, assume cartesian mapping on 2D homology.
%
% This is to the best of my knowledge and ability a good and reasonable
% estimation of error based on cameras position, focal legnth, dpi, etc.
%
% ===========================================================================================

clear all
close all

% This model is a function of 6 parameters, Sector angle, Camera position, Camera
% quality, Camera FOV, Landing point, precieved landing point

% Official Sector measurements

SectorHalfAngle = 17.46; % degrees
circleDiameter = 2.135;  % meters (shot/discus/hammer circle diameter; edit if needed)
Rcircle = circleDiameter/2;

% Camera position (x,y,z) Assuming the middle of the throwing circle is at
% (0,0), the y axis is the centerline toward where the implement is
% thrown, and the x axis is toward the third camera
% ================================================================
%                 TOP VIEW (GROUND PLANE: z = 0)
%
%                        +y  (Throw Direction)
%                        ^
%                        |
%      Camera 2 o        |        o Camera 1
%                \       |       /
%                 \      O Landing point
%                  \     |     /
%                   \    |    /
%                    \   |   /
%                     \  |  /
%                      \ | /
%                       \|/ o Camera 3
%        ----------------O------------------> +x
%                  Throwing Circle(0,0)
%
%        Sector lines symmetric about +y axis
%
% ================================================================
L = 30;            % meters along the sector boundary line from origin
xBound = L * sind(SectorHalfAngle);
yBound = L * cosd(SectorHalfAngle);
tripodheight = 2;

Camera1 = [xBound,yBound,tripodheight];
Camera2 = [-xBound,yBound,tripodheight];
Camera3 = [1.5,1,tripodheight];


% Camera quality, common qualities 3840 x 2160 pixels, 1920 x 1440 pixels, 1920 x 1080 pixels

xRes = 1920;
yRes = 1080;

% Maximum expected landing distance (meters) Shot Put 25, Discus 75, Hammer Throw 85, Javelin 100

LandingSpot = [0,20,0];

% FOV of all cameras, vertical and horizontal, from GoPro's Website
% ================================================================
% FOV Settings (degrees)
%
% Setting                     | V.FOV (deg) | H.FOV (deg) | Diag. FOV (deg)
% ---------------------------|-------------|-------------|----------------
% 4 x 3 W   (zoom = 0%)      |   94.4      |   122.6     |   149.2
% 4 x 3 W   (zoom = 100%)    |   49.1      |    64.6     |    79.7
% 16 x 9 W  (zoom = 0%)      |   69.5      |   118.2     |   133.6
% 16 x 9 W  (zoom = 100%)    |   35.7      |    62.2     |    70.8
% 16 x 9 Linear (zoom = 0%)  |   55.2      |    85.5     |    N/A
% 16 x 9 Linear (zoom = 100%)|   29.3      |    49.8     |    N/A CURRENT 
% ================================================================

VFOV = 29.3; % degrees
HFOV = 49.8; % degrees 

% Change precieved landing location for each camera so we can get better 
% pixel density near the camera and less horizon in view
% The offset works as a percentage, how close the precieved landing point is 
% to the real landing point. (0 = original, 1 = at camera)
PrecievedLandingSpot = .55;


%%
% ================================================================
% PROCEDURE
% First we will aim all our cameras at the expected landing point.
% Then we will calculate if the camera's are far enough away to 
% at least tangent the throwing sector to ensure we do not miss 
% any implements near our expected throw distance. Then we will map each
% pixel of our camera to a square on the ground using our phi and theta 
% angles. All these maps will be overlayed on top of each other, and the
% smallest pixel for any given area will be displayed on a color map
% with large pixels being red, and small pixels being green. Then we will
% draw the throwing circle, secotr, and colormap on a figure. This script
% could easily be functionalized and maxamized over a certain value to find
% the best parameters.
% ================================================================

Cams = [Camera1; Camera2; Camera3];
nCams = size(Cams, 1); % Number of cameras
CamAim = zeros(3);

fprintf('  Landing spot  [x y z] = [%.2f %.2f %.2f] m\n', LandingSpot(1),LandingSpot(2),LandingSpot(3));


for i = 1:nCams

    C = Cams(i,:);

    NewLanding = C + (1 - PrecievedLandingSpot)*(LandingSpot - C);
    NewLanding(3) = 0;

    [yawDeg, pitchDeg] = pointAnglesYawPitch(C, NewLanding);
    CamAim(i,:) = [yawDeg, pitchDeg, norm(LandingSpot-C)];

    fprintf('  Camera %u', i);
    fprintf('  Camera pos [x y z] = [%.2f %.2f %.2f] m\n', C(1),C(2),C(3));
    fprintf('  Aiming at [%.2f %.2f %.2f] m\n', NewLanding(1),NewLanding(2),NewLanding(3));
    fprintf('  Yaw   = %+7.3f deg  (about +z, 0=+x, +90=+y)\n', yawDeg);
    fprintf('  Pitch = %+7.3f deg  (about camera x-right axis; + up)\n', pitchDeg);
    fprintf('  Range = %7.3f m\n\n', norm(LandingSpot-C));

    [u_new, u_horizon] = verticalFrameMetrics(C, yawDeg, pitchDeg, NewLanding, VFOV);

    fprintf('  New landing vertical position in frame u = %.3f (0=top, 0.5=center, 1=bottom)\n', u_new);
    fprintf('  Horizon vertical position in frame u     = %.3f (<0 no horizon visible)\n\n', u_horizon);
end

% --- PLOT SETUP ---
figure; clf; hold on; axis equal; grid on;
xlabel('x (m)'); ylabel('y (m)');
title('Throw Sector + Camera Ground-Plane FOV Footprints');
theme("light")

% Throwing circle
th = linspace(0,2*pi,400);
hCircle = plot(Rcircle*cos(th), Rcircle*sin(th), 'k-', 'LineWidth', 2, ...
               'DisplayName', 'Throwing circle');

% Sector boundary rays: +/- sectorHalfAngle from +y axis
% Convert to world angle measured from +x: phi = 90deg +/- sectorHalfAngle
phiL = deg2rad(90 + SectorHalfAngle);
phiR = deg2rad(90 - SectorHalfAngle);
rayLen = 120; % meters (safe default; adjust)
hSector = plot([0, rayLen*cos(phiL)], [0, rayLen*sin(phiL)], ...
               'k--', 'LineWidth', 1.5, 'DisplayName', 'Sector boundary');

plot([0, rayLen*cos(phiR)], [0, rayLen*sin(phiR)], ...
     'k--', 'LineWidth', 1.5, 'HandleVisibility', 'off');   % no legend entry

% Origin & landing spot
hOrigin = plot(0,0,'ko','MarkerFaceColor','k','MarkerSize',6, ...
               'DisplayName', 'Origin');

hLand = plot(LandingSpot(1),LandingSpot(2),'kx','LineWidth',2,'MarkerSize',10, ...
             'DisplayName', 'Expected Landing');

% --- PLOT EACH CAMERA FOOTPRINT ---
colors = lines(nCams);
camH = gobjects(nCams,1);   % handles for legend (put this BEFORE the loop)

maxXY = 0;
for i = 1:nCams
    C = Cams(i,:);
    yawDeg   = CamAim(i,1);
    pitchDeg = CamAim(i,2);

    % Compute ground footprint corners (4x3) and ellipse params
    [Pxy, clippedCorner] = groundFootprintRect(C, yawDeg, pitchDeg, HFOV, VFOV, L);

    % Fill quad (no edge; we'll draw edges manually)
    patch('XData', Pxy(:,1), 'YData', Pxy(:,2), ...
          'FaceColor', colors(i,:), 'FaceAlpha', 0.18, ...
          'EdgeColor', 'none', 'HandleVisibility', 'off');
    
    % Draw edges: dotted if that edge touches any clipped corner
    for e = 1:4
        i1 = e;
        i2 = mod(e,4) + 1;
    
        isClippedEdge = clippedCorner(i1) && clippedCorner(i2);
    
        if isClippedEdge
            ls = ':';      % dotted
            lw = 1.5;      % slightly thinner
        else
            ls = '-';      % solid
            lw = 2.5;      % thick
        end
    
        plot(Pxy([i1 i2],1), Pxy([i1 i2],2), ...
             'LineStyle', ls, 'LineWidth', lw, 'Color', colors(i,:), ...
             'HandleVisibility', 'off');
    end
    camH(i) = plot(C(1), C(2), 'o', ...
    'Color', colors(i,:), 'MarkerFaceColor', colors(i,:), ...
    'MarkerSize', 7, 'DisplayName', sprintf('Cam%d', i));
    text(C(1), C(2), sprintf('  Cam%d', i), 'Color', colors(i,:));

    % Track plot bounds
    maxXY = max([maxXY; abs(Pxy(:)); abs(C(1)); abs(C(2))]);
end

% Nice bounds
lim = max(15, maxXY*1.1);
xlim([-lim, lim]);
ylim([-2, lim]);  % usually only need forward (+y)

legend([hCircle, hSector, hOrigin, hLand, camH(:).'], 'Location', 'northwest');



%%
% =================================================================
%                          FUNCTIONS
% =================================================================
% Function: yaw/pitch needed for camera at C to look at target P
% World axes: +x right, +y downfield, +z up
%
% Returns:
% Yaw: angle in ground plane from +x toward +y
% Pitch: elevation angle above horizon (positive = up); negative = looking down
% Range: Norm of distance
% ================================================================

function [yawDeg, pitchDeg] = pointAnglesYawPitch(C, P)
    v = P - C;                    % vector from camera to target

    % Yaw in x-y plane
    yaw = atan2(v(2), v(1));      % arctan in any quadrant
    yawDeg = rad2deg(yaw);

    % Pitch: angle above horizontal plane
    horiz = hypot(v(1), v(2));    % find hypotenuse
    pitch = atan2(v(3), horiz);   % radians
    pitchDeg = rad2deg(pitch);
end

% ================================================================
% Function: Quantifies where the NEW landing location appears vertically in the 
% camera frame and where the horizon appears in the frame.
%
% Returns:
% u_new: Normalized vertical frame position of NewLanding
% (0 = top of frame, 0.5 = center, 1 = bottom)
%
% u_horizon: Normalized vertical frame position of the horizon
% (0 = top of frame, 0.5 = center, 1 = bottom)
% ================================================================

function [u_new, u_horizon] = verticalFrameMetrics(C, yawDeg, pitchDeg, NewLanding, VFOVdeg)

    % World up vector
    upW = [0 0 1];

    % Convert angles
    psi   = deg2rad(yawDeg);
    theta = deg2rad(pitchDeg);

    % Camera forward (optical axis)
    fwd = [cos(theta)*cos(psi), cos(theta)*sin(psi), sin(theta)];
    fwd = fwd / norm(fwd);

    % Camera basis
    right = cross(fwd, upW);
    right = right / norm(right);
    up = cross(right, fwd);
    up = up / norm(up);

    % ----- 1) NEW landing point vertical position -----
    v = NewLanding - C;
    v = v / norm(v);

    vAngle = atan2(dot(v, up), dot(v, fwd));   % radians
    vAngleDeg = rad2deg(vAngle);

    u_new = 0.5 - vAngleDeg / VFOVdeg;


    % ----- 2) Horizon vertical position -----
    % Horizon occurs when vertical ray angle = 0 elevation
    % In this coordinate system, horizon offset = -pitch
    u_horizon = 0.5 + pitchDeg / VFOVdeg;

end

% ================================================================
% Function: Ground footprint using HFOV/VFOV and yaw/pitch
% Returns:
% Pxy = 4x2 polygon corners on ground (in order)
% clippedCorner = Shows all corner on horizion
% ================================================================

function [Pxy, clippedCorner] = groundFootprintRect(C, yawDeg, pitchDeg, HFOVdeg, VFOVdeg, L)

    Maxdist = L;
    upW = [0 0 1];

    psi   = deg2rad(yawDeg);
    theta = deg2rad(pitchDeg);

    % Forward direction (optical axis)
    fwd = [cos(theta)*cos(psi), cos(theta)*sin(psi), sin(theta)];
    fwd = fwd / norm(fwd);

    % Camera right/up basis
    right = cross(fwd, upW);
    if norm(right) < 1e-9
        right = [1 0 0];
    end
    right = right / norm(right);

    up = cross(right, fwd);
    up = up / norm(up);

    % Half-angle tangents
    hx = tan(deg2rad(HFOVdeg/2));
    hy = tan(deg2rad(VFOVdeg/2));

    % Corner combinations (ordered around quad)
    s = [ -1 -1;
          +1 -1;
          +1 +1;
          -1 +1 ];

    P = zeros(4,3);
    clippedCorner = false(4,1);

    for k = 1:4
        sx = s(k,1);
        sy = s(k,2);

        dir = fwd + sx*hx*right + sy*hy*up;
        dir = dir / norm(dir);

        % Intersect with ground (z=0): t = -Cz/dirz
        if abs(dir(3)) > 1e-12
            t = -C(3)/dir(3);
        else
            t = -1; % force clip
        end

        if t > 0
            Ptemp = C + t*dir;
        else
            % CLIP: extend ray in XY until horizontal distance hits Maxdist
            clippedCorner(k) = true;

            dirXY = dir(1:2);
            if norm(dirXY) < 1e-12
                dirXY = [1 0]; % degenerate fallback
            end
            dirXY = dirXY / norm(dirXY);

            PtempXY = C(1:2) + Maxdist * dirXY;
            Ptemp = [PtempXY, 0];
        end

        P(k,:) = Ptemp;
    end

    Pxy = P(:,1:2);
end
